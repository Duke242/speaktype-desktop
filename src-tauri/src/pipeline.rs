//! Audio in, cleaned-up text out: shared by dictation and file transcription.

use std::{path::PathBuf, sync::MutexGuard};

use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, history::HistoryItem, media, models, text, transcribe::Engine};

pub enum Error {
    NoModel,
    ModelLoad(String),
    Transcribe(String),
    NoSpeech,
}

impl Error {
    /// Short message for the pill and the UI.
    pub fn message(&self) -> &'static str {
        match self {
            Error::NoModel => "No model selected",
            Error::ModelLoad(_) => "Model load failed",
            Error::Transcribe(_) => "Transcription failed",
            Error::NoSpeech => "No speech detected",
        }
    }

    pub fn detail(&self) -> Option<&str> {
        match self {
            Error::ModelLoad(e) | Error::Transcribe(e) => Some(e),
            _ => None,
        }
    }
}

/// Transcribes 16 kHz mono audio with the selected model, saves the recording
/// and the result to history, and returns the history item.
///
/// `on_warming` is called with `true` before the model starts loading and
/// `false` once transcription begins.
pub fn run(
    app: &AppHandle,
    samples: &[f32],
    duration_secs: f64,
    on_warming: impl Fn(bool),
) -> Result<HistoryItem, Error> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let model = models::find(&settings.selected_model)
        .filter(|m| state.models.is_downloaded(m.id))
        .ok_or(Error::NoModel)?;

    let mut engine = lock_engine(&state, &on_warming);
    if engine.loaded_model() != Some(model.id) {
        on_warming(true);
        state
            .load_model(app, &mut engine, model)
            .map_err(Error::ModelLoad)?;
        on_warming(false);
    }
    let raw = engine
        .transcribe(samples, &settings.language)
        .map_err(Error::Transcribe)?;
    drop(engine);

    let text = text::process(
        &raw,
        &text::Options {
            auto_edit: settings.auto_edit,
            smart_trailing_punctuation: settings.smart_trailing_punctuation,
            dictionary: &settings.dictionary,
        },
    );
    if text.is_empty() {
        return Err(Error::NoSpeech);
    }

    let mut history = state.history.lock().unwrap();
    let audio_path = save_recording(history.recordings_dir().to_path_buf(), samples);
    let item = history
        .add(&text, duration_secs, model.name, audio_path)
        .ok()
        .flatten();
    drop(history);
    let _ = app.emit("history-changed", ());

    // Saving is best effort: the text is still returned if the disk write failed.
    Ok(item.unwrap_or_else(|| HistoryItem {
        id: String::new(),
        created_at: crate::history::now_ms(),
        word_count: text.split_whitespace().count(),
        transcript: text,
        duration_secs,
        model: model.name.into(),
        audio_path: None,
    }))
}

/// If a warm-up holds the engine, the model is still loading, so report that while waiting.
fn lock_engine<'a>(state: &'a AppState, on_warming: &impl Fn(bool)) -> MutexGuard<'a, Engine> {
    match state.engine.try_lock() {
        Ok(engine) => engine,
        Err(_) => {
            on_warming(true);
            let engine = state.engine.lock().unwrap_or_else(|e| e.into_inner());
            on_warming(false);
            engine
        }
    }
}

fn save_recording(dir: PathBuf, samples: &[f32]) -> Option<PathBuf> {
    let path = dir.join(format!("recording-{}.wav", crate::history::now_ms()));
    match media::write_wav(&path, samples) {
        Ok(()) => Some(path),
        Err(e) => {
            eprintln!("[history] couldn't save recording: {e}");
            None
        }
    }
}
