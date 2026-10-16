//! Whisper through whisper.cpp.
//!
//! With the language set to "auto", whisper.cpp detects the language once, from
//! the start of the audio, and writes everything after in that language. So
//! that people who switch languages between sentences get each one in its own
//! language, longer dictations are split at pauses and each part is detected
//! on its own. Switching within a sentence, with no pause, still comes out in
//! one language: whisper.cpp can't detect more often than once per call, and
//! very short parts are too little to tell languages apart reliably.

use std::{borrow::Cow, ops::Range, path::Path};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use crate::audio::WHISPER_SAMPLE_RATE;

/// whisper.cpp skips input shorter than one second, so short clips are padded
/// with silence up to this length.
const MIN_SAMPLES: usize = WHISPER_SAMPLE_RATE as usize * 5 / 4;
/// Whisper reads at most half its 448-token text context as a prompt.
const MAX_PROMPT_TOKENS: usize = 224;

const FRAME: usize = WHISPER_SAMPLE_RATE as usize / 50;
/// Auto-detect: each part detected on its own is at least this long, since
/// language detection on a second or two of speech is unreliable.
const MIN_PART: usize = WHISPER_SAMPLE_RATE as usize * 5 / 2;
/// Auto-detect: a pause this long can separate two languages.
const MIN_PAUSE: usize = WHISPER_SAMPLE_RATE as usize * 2 / 5;
/// Frames quieter than this RMS are a pause, as in `live`.
const QUIET_RMS: f32 = 0.006;
/// Each part costs a pass of the encoder, so long dictations with many pauses
/// are split into at most this many.
const MAX_PARTS: usize = 6;

pub struct Whisper {
    // Kept alive for the state, which borrows the model weights, and used to
    // tokenize prompts.
    context: WhisperContext,
    /// Created once per model. With CoreML, creating a state loads the Neural
    /// Engine model, which is far too slow to repeat for every dictation.
    state: WhisperState,
}

impl Whisper {
    pub fn load(path: &Path) -> Result<Self, String> {
        // whisper-rs turns flash attention off, unlike whisper.cpp. On, the GPU
        // is about a quarter faster (Metal, M1 Max) and the CPU and Neural
        // Engine are unchanged.
        let mut params = WhisperContextParameters::default();
        params.flash_attn(true);
        let context = WhisperContext::new_with_params(path, params)
            .map_err(|e| format!("Couldn't load model: {e}"))?;
        let state = context
            .create_state()
            .map_err(|e| format!("Couldn't prepare model: {e}"))?;
        Ok(Self { context, state })
    }

    /// `prompt` is text Whisper treats as coming just before this audio: words to
    /// expect and, while typing live, what was already said. Empty for none.
    pub fn transcribe(
        &mut self,
        samples: &[f32],
        language: &str,
        prompt: &str,
    ) -> Result<String, String> {
        let prompt_tokens = self.prompt_tokens(prompt);
        if language != "auto" {
            return self.run(samples, Some(language), &prompt_tokens);
        }
        // English-only models have nothing to detect.
        if !self.context.is_multilingual() {
            return self.run(samples, None, &prompt_tokens);
        }
        let parts = parts(samples);
        let mut texts = Vec::with_capacity(parts.len());
        let mut languages = Vec::with_capacity(parts.len());
        for part in parts {
            let text = self.run(&samples[part], None, &prompt_tokens)?;
            languages.push(whisper_rs::get_lang_str(
                self.state.full_lang_id_from_state(),
            ));
            if !text.trim().is_empty() {
                texts.push(text.trim().to_string());
            }
        }
        if languages.windows(2).any(|pair| pair[0] != pair[1]) {
            let languages: Vec<_> = languages.iter().map(|l| l.unwrap_or("?")).collect();
            eprintln!("[whisper] detected {}", languages.join(", "));
        }
        Ok(texts.join(" "))
    }

    /// Tokenized here rather than with `set_initial_prompt`, which leaks its
    /// copy of the string. A prompt that won't tokenize is dropped, not fatal.
    fn prompt_tokens(&self, prompt: &str) -> Vec<i32> {
        if prompt.is_empty() {
            return Vec::new();
        }
        // The buffer must fit every token: when it's too small, whisper.cpp
        // returns minus the count, which whisper-rs takes as a length.
        // Whisper's tokens are byte-level, so there are never more than bytes.
        let mut tokens = self
            .context
            .tokenize(prompt, prompt.len() + 1)
            .unwrap_or_else(|e| {
                eprintln!("[whisper] ignoring prompt: {e}");
                Vec::new()
            });
        // Like whisper.cpp, keep the end: the words just said.
        tokens.drain(..tokens.len().saturating_sub(MAX_PROMPT_TOKENS));
        tokens
    }

    /// One whisper.cpp pass. `language` None detects it from this audio.
    fn run(
        &mut self,
        samples: &[f32],
        language: Option<&str>,
        prompt_tokens: &[i32],
    ) -> Result<String, String> {
        // Greedy decoding at temperature 0 with whisper.cpp's usual temperature
        // fallback, matching the defaults the macOS app gets from WhisperKit.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        if !prompt_tokens.is_empty() {
            params.set_tokens(prompt_tokens);
        }
        params.set_language(language);
        params.set_n_threads(thread_count());
        params.set_no_context(true);
        params.set_no_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let audio = if samples.len() < MIN_SAMPLES {
            let mut padded = samples.to_vec();
            padded.resize(MIN_SAMPLES, 0.0);
            Cow::Owned(padded)
        } else {
            Cow::Borrowed(samples)
        };
        self.state.full(params, &audio).map_err(|e| e.to_string())?;

        Ok(self
            .state
            .as_iter()
            .filter_map(|segment| segment.to_str_lossy().ok().map(Cow::into_owned))
            .collect::<Vec<_>>()
            .join(" "))
    }
}

/// Splits audio at pauses into parts whose language is detected separately:
/// each at least `MIN_PART` long, cut in the middle of a pause of at least
/// `MIN_PAUSE`, and at most `MAX_PARTS` of them. Covers all of `samples`.
fn parts(samples: &[f32]) -> Vec<Range<usize>> {
    let quiet: Vec<bool> = samples
        .chunks(FRAME)
        .map(|frame| {
            let energy = frame.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
            ((energy / frame.len() as f64).sqrt() as f32) < QUIET_RMS
        })
        .collect();
    // Middles of long enough pauses, in samples.
    let mut cuts = Vec::new();
    let mut run_start = None;
    for (index, &is_quiet) in quiet.iter().chain([&false]).enumerate() {
        match (is_quiet, run_start) {
            (true, None) => run_start = Some(index),
            (false, Some(start)) => {
                if (index - start) * FRAME >= MIN_PAUSE {
                    cuts.push((start + index) / 2 * FRAME);
                }
                run_start = None;
            }
            _ => {}
        }
    }

    let mut parts = Vec::new();
    let mut start = 0;
    for cut in cuts {
        let cut = cut.min(samples.len());
        if cut - start >= MIN_PART && samples.len() - cut >= MIN_PART {
            parts.push(start..cut);
            start = cut;
        }
    }
    parts.push(start..samples.len());
    // Too many parts: join neighbours, shortest pair first, until few enough.
    while parts.len() > MAX_PARTS {
        let index = (0..parts.len() - 1)
            .min_by_key(|&i| parts[i + 1].end - parts[i].start)
            .unwrap_or(0);
        let next = parts.remove(index + 1);
        parts[index].end = next.end;
    }
    parts
}

/// whisper.cpp gains little past 8 threads, and more would starve the rest of the system.
fn thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

/// Routes whisper.cpp's verbose logging away from stderr.
pub fn silence_logs() {
    whisper_rs::install_logging_hooks();
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: usize = WHISPER_SAMPLE_RATE as usize;

    /// Speech and pauses, each given in tenths of a second.
    fn audio(pattern: &[(bool, usize)]) -> Vec<f32> {
        pattern
            .iter()
            .flat_map(|&(speech, tenths)| vec![if speech { 0.05 } else { 0.0 }; RATE * tenths / 10])
            .collect()
    }

    fn assert_whole(samples: &[f32]) {
        let parts = parts(samples);
        assert_eq!(parts.len(), 1, "{parts:?}");
        assert_covers(&parts, samples.len());
    }

    fn assert_covers(parts: &[Range<usize>], len: usize) {
        assert_eq!(parts.first().map(|p| p.start), Some(0));
        assert_eq!(parts.last().map(|p| p.end), Some(len));
        for pair in parts.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "{parts:?}");
        }
    }

    #[test]
    fn sentences_separated_by_a_pause_are_detected_separately() {
        let samples = audio(&[(true, 30), (false, 6), (true, 30)]);
        let parts = parts(&samples);
        assert_eq!(parts.len(), 2, "{parts:?}");
        // Cut in the middle of the pause.
        let cut = parts[0].end as f32 / RATE as f32;
        assert!((cut - 3.3).abs() < 0.03, "cut at {cut}s");
        assert_covers(&parts, samples.len());
    }

    #[test]
    fn short_dictations_and_short_pauses_stay_whole() {
        let short = audio(&[(true, 20), (false, 6), (true, 20)]);
        assert_whole(&short);
        let breath = audio(&[(true, 30), (false, 2), (true, 30)]);
        assert_whole(&breath);
        assert_whole(&[]);
    }

    #[test]
    fn leading_and_trailing_silence_never_make_a_part_of_their_own() {
        assert_whole(&audio(&[(false, 10), (true, 30), (false, 15)]));
    }

    #[test]
    fn long_dictations_with_many_pauses_are_capped() {
        let pattern: Vec<_> = (0..20).flat_map(|_| [(true, 30), (false, 5)]).collect();
        let samples = audio(&pattern);
        let parts = parts(&samples);
        assert_eq!(parts.len(), MAX_PARTS);
        assert_covers(&parts, samples.len());
        assert!(parts.iter().all(|p| p.len() >= MIN_PART));
    }
}
