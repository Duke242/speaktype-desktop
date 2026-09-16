//! Speech-to-text with whisper.cpp.

use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio::WHISPER_SAMPLE_RATE;

/// whisper.cpp skips input shorter than one second, so short clips are padded
/// with silence up to this length.
const MIN_SAMPLES: usize = WHISPER_SAMPLE_RATE as usize * 5 / 4;

#[derive(Default)]
pub struct Engine {
    loaded: Option<(String, WhisperContext)>,
}

impl Engine {
    pub fn loaded_model(&self) -> Option<&str> {
        self.loaded.as_ref().map(|(id, _)| id.as_str())
    }

    /// Loads a model unless it is already the loaded one. The previous model is
    /// freed first so two large models are never in memory together.
    pub fn load(&mut self, id: &str, path: &Path) -> Result<(), String> {
        if self.loaded_model() == Some(id) {
            return Ok(());
        }
        self.loaded = None;
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| format!("Couldn't load model: {e}"))?;
        self.loaded = Some((id.to_string(), context));
        Ok(())
    }

    pub fn unload(&mut self) {
        self.loaded = None;
    }

    /// Transcribes 16 kHz mono audio. `language` is a Whisper code or "auto".
    pub fn transcribe(&self, samples: &[f32], language: &str) -> Result<String, String> {
        let (_, context) = self.loaded.as_ref().ok_or("No model loaded")?;
        let mut state = context.create_state().map_err(|e| e.to_string())?;

        // Greedy decoding at temperature 0 with whisper.cpp's usual temperature
        // fallback, matching the defaults the macOS app gets from WhisperKit.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
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

        let mut audio = samples.to_vec();
        if audio.len() < MIN_SAMPLES {
            audio.resize(MIN_SAMPLES, 0.0);
        }
        state.full(params, &audio).map_err(|e| e.to_string())?;

        let text = state
            .as_iter()
            .filter_map(|segment| segment.to_str_lossy().ok().map(|s| s.into_owned()))
            .collect::<Vec<_>>()
            .join(" ");
        Ok(text)
    }
}

fn thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

/// Routes whisper.cpp's verbose logging away from stderr.
pub fn silence_logs() {
    whisper_rs::install_logging_hooks();
}
