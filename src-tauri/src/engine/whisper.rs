//! Whisper through whisper.cpp.

use std::{borrow::Cow, path::Path};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use crate::audio::WHISPER_SAMPLE_RATE;

/// whisper.cpp skips input shorter than one second, so short clips are padded
/// with silence up to this length.
const MIN_SAMPLES: usize = WHISPER_SAMPLE_RATE as usize * 5 / 4;
/// Whisper reads at most half its 448-token text context as a prompt.
const MAX_PROMPT_TOKENS: usize = 224;

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
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
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
        // Tokenized here rather than with `set_initial_prompt`, which leaks its
        // copy of the string. A prompt that won't tokenize is dropped, not fatal.
        let prompt_tokens = if prompt.is_empty() {
            Vec::new()
        } else {
            self.context
                .tokenize(prompt, MAX_PROMPT_TOKENS)
                .unwrap_or_else(|e| {
                    eprintln!("[whisper] ignoring prompt: {e}");
                    Vec::new()
                })
        };
        // Greedy decoding at temperature 0 with whisper.cpp's usual temperature
        // fallback, matching the defaults the macOS app gets from WhisperKit.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        if !prompt_tokens.is_empty() {
            params.set_tokens(&prompt_tokens);
        }
        params.set_language(if language == "auto" {
            None
        } else {
            Some(language)
        });
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
