//! Audio in, cleaned-up text out: shared by dictation and file transcription.

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::MutexGuard,
};

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    AppState, LockExt,
    cleanup::{self, Cleaner},
    engine::Engine,
    gain,
    history::{self, HistoryItem},
    live, media, models,
    platform::FrontApp,
    text, tone,
};

#[derive(Debug)]
pub enum Error {
    NoModel,
    ModelLoad(String),
    Transcribe(String),
    NoSpeech,
    /// Command mode couldn't edit the selection: a message for the pill, and
    /// what went wrong, for the log.
    Command(&'static str, Option<String>),
}

impl Error {
    /// Short message for the pill and the UI.
    pub fn message(&self) -> &'static str {
        match self {
            Error::NoModel => "No model selected",
            Error::ModelLoad(_) => "Model load failed",
            Error::Transcribe(_) => "Transcription failed",
            Error::NoSpeech => "No speech detected",
            Error::Command(message, _) => message,
        }
    }

    /// The underlying engine error, for logs and the Transcribe Audio screen.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Error::ModelLoad(e) | Error::Transcribe(e) => Some(e),
            Error::Command(_, detail) => detail.as_deref(),
            Error::NoModel | Error::NoSpeech => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.detail() {
            Some(detail) => write!(f, "{}: {detail}", self.message()),
            None => f.write_str(self.message()),
        }
    }
}

impl std::error::Error for Error {}

/// What the pipeline is busy with, for the pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Waiting for the speech model to load.
    Loading,
    Transcribing,
    /// A local language model is tidying the transcript.
    CleaningUp,
    /// A local language model is editing the selection, in command mode.
    Editing,
}

/// Transcribes 16 kHz mono audio with the selected model, saves the recording
/// and the result to history, and returns the history item.
///
/// `on_stage` is called as the work moves along. Loading is always followed by
/// Transcribing, whether or not loading succeeded. `target` is the app being
/// dictated into, which sets the tone.
pub fn run(
    app: &AppHandle,
    samples: &[f32],
    duration_secs: f64,
    target: Option<&FrontApp>,
    on_stage: impl Fn(Stage),
) -> Result<HistoryItem, Error> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let model = models::find(&settings.selected_model)
        .filter(|m| state.models.is_downloaded(m.id))
        .ok_or(Error::NoModel)?;

    let mut engine = lock_engine(&state, &on_stage);
    if engine.loaded_model() != Some(model.id) {
        on_stage(Stage::Loading);
        let loaded = state.load_model(app, &mut engine, model);
        on_stage(Stage::Transcribing);
        loaded.map_err(Error::ModelLoad)?;
    }
    // Whisper mode. The recording is saved as it was captured.
    let heard = if settings.boost_quiet_speech {
        gain::boost(samples)
    } else {
        samples.into()
    };
    // Given words to expect and only silence, Whisper may write those words.
    let prompt = if live::has_speech(&heard) {
        text::prompt(&settings.dictionary, "")
    } else {
        String::new()
    };
    let raw = engine
        .transcribe(&heard, &settings.language, &prompt)
        .map_err(Error::Transcribe)?;
    drop(engine);

    let options = text::Options {
        auto_edit: settings.auto_edit,
        smart_trailing_punctuation: settings.smart_trailing_punctuation,
        plain_punctuation: settings.plain_punctuation,
        dictionary: &settings.dictionary,
        voice_commands: settings.voice_commands,
        self_corrections: settings.self_corrections,
        smart_formatting: settings.smart_formatting,
        snippets: &settings.snippets,
        english: text::is_english(&settings.language),
    };
    let style = tone::Style::new(&settings.tone, target);
    let mut text = style.format(&text::normalize_transcription(
        &raw,
        options.auto_edit,
        options.plain_punctuation,
    ));
    if Cleaner::will_run(&settings.cleanup, &state.models, &text) {
        on_stage(Stage::CleaningUp);
        let mut hint = style.hint();
        if options.plain_punctuation {
            hint = Some(cleanup::plain_punctuation(hint));
        }
        text = state
            .cleaner
            .clean_for(&settings.cleanup, &state.models, &text, hint.as_ref());
    } else {
        text = style.adjust(&text);
    }
    // After cleanup, so dictionary rules and snippets always have the last word.
    let text = text::finish(&text, &options);
    if text.is_empty() {
        return Err(Error::NoSpeech);
    }

    Ok(save(app, samples, duration_secs, &text, model.name))
}

/// Transcribes audio as `run` does, without cleanup or saving it: for
/// command mode, where what was said is an instruction. Returns the text and
/// the name of the model that heard it.
pub fn transcribe_only(
    app: &AppHandle,
    samples: &[f32],
    on_stage: impl Fn(Stage),
) -> Result<(String, &'static str), Error> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    // Whisper mode, as for dictation.
    let samples = &*if settings.boost_quiet_speech {
        gain::boost(samples)
    } else {
        samples.into()
    };
    // Whisper may hear words in silence, which mustn't become an edit.
    if !live::has_speech(samples) {
        return Err(Error::NoSpeech);
    }
    let model = models::find(&settings.selected_model)
        .filter(|m| state.models.is_downloaded(m.id))
        .ok_or(Error::NoModel)?;
    let mut engine = lock_engine(&state, &on_stage);
    if engine.loaded_model() != Some(model.id) {
        on_stage(Stage::Loading);
        let loaded = state.load_model(app, &mut engine, model);
        on_stage(Stage::Transcribing);
        loaded.map_err(Error::ModelLoad)?;
    }
    let raw = engine
        .transcribe(samples, &settings.language, "")
        .map_err(Error::Transcribe)?;
    drop(engine);
    // An instruction for the model, not text to paste, so its punctuation stays.
    let text = text::normalize_transcription(&raw, true, false);
    if text.trim().is_empty() {
        return Err(Error::NoSpeech);
    }
    Ok((text.trim().to_string(), model.name))
}

/// Save one history entry for the entire live session, rather than one per phrase.
pub fn save(
    app: &AppHandle,
    samples: &[f32],
    duration_secs: f64,
    text: &str,
    model_name: &str,
) -> HistoryItem {
    let state = app.state::<AppState>();

    // The WAV is written without holding the history lock, so the UI can still
    // read history meanwhile.
    let audio_path = if state.settings().save_recordings {
        let recordings_dir = state
            .history
            .lock_unpoisoned()
            .recordings_dir()
            .to_path_buf();
        save_recording(&recordings_dir, samples)
    } else {
        None
    };
    let added = state
        .history
        .lock_unpoisoned()
        .add(text, duration_secs, model_name, audio_path);
    let _ = app.emit("history-changed", ());

    // Saving is best effort: the text is still returned if the disk write failed.
    let item = match added {
        Ok(item) => item,
        Err(e) => {
            eprintln!("[history] couldn't save transcript: {e}");
            None
        }
    };
    item.unwrap_or_else(|| HistoryItem {
        id: String::new(),
        created_at: history::now_ms(),
        word_count: text.split_whitespace().count(),
        transcript: text.to_string(),
        duration_secs,
        model: model_name.into(),
        audio_path: None,
    })
}

/// If a warm-up holds the engine, the model is still loading, so report that while waiting.
fn lock_engine<'a>(state: &'a AppState, on_stage: &impl Fn(Stage)) -> MutexGuard<'a, Engine> {
    if let Some(engine) = state.engine.try_lock_unpoisoned() {
        return engine;
    }
    on_stage(Stage::Loading);
    let engine = state.engine.lock_unpoisoned();
    on_stage(Stage::Transcribing);
    engine
}

fn save_recording(dir: &Path, samples: &[f32]) -> Option<PathBuf> {
    let path = dir.join(format!("recording-{}.wav", history::now_ms()));
    match media::write_wav(&path, samples) {
        Ok(()) => Some(path),
        Err(e) => {
            eprintln!("[history] couldn't save recording: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn errors_show_their_detail_when_they_have_one() {
        assert_eq!(Error::NoModel.to_string(), "No model selected");
        assert_eq!(Error::NoSpeech.to_string(), "No speech detected");
        assert_eq!(
            Error::ModelLoad("file is corrupt".into()).to_string(),
            "Model load failed: file is corrupt"
        );
        assert_eq!(
            Error::Transcribe("out of memory".into()).to_string(),
            "Transcription failed: out of memory"
        );
    }

    #[test]
    fn short_messages_leave_out_the_detail() {
        let error = Error::Transcribe("out of memory".into());
        assert_eq!(error.message(), "Transcription failed");
        assert_eq!(error.detail(), Some("out of memory"));
        assert_eq!(Error::NoSpeech.detail(), None);
    }
}
