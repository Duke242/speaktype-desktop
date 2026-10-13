//! User settings, stored as JSON in the app's config directory.
//!
//! Every field falls back to its default when missing, so settings files from
//! older versions keep loading as fields are added.

use std::{
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::text::DictionaryEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordingMode {
    /// Record while the hotkey is held, transcribe on release.
    Hold,
    /// Press once to start, again to stop.
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PillPosition {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupBackend {
    /// A model Wordfall downloads and runs itself.
    Builtin,
    /// A local server with an OpenAI-compatible API, such as Ollama or LM Studio.
    Server,
}

/// Optional tidying of each transcript by a local language model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CleanupSettings {
    pub enabled: bool,
    pub backend: CleanupBackend,
    /// Id from `models::CLEANUP_CATALOG`, or empty when nothing is chosen yet.
    pub builtin_model: String,
    /// Base URL of the server's OpenAI-compatible API, e.g. "http://localhost:11434/v1".
    pub server_url: String,
    /// Model name as the server knows it, e.g. "llama3.2".
    pub server_model: String,
    /// How long Ollama keeps the model in memory after a dictation, in
    /// minutes. Negative keeps it until Ollama quits.
    pub server_keep_alive_minutes: i32,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: CleanupBackend::Builtin,
            builtin_model: String::new(),
            server_url: "http://localhost:11434/v1".to_string(),
            server_model: String::new(),
            server_keep_alive_minutes: 60,
        }
    }
}

/// Meeting notes (see `meeting`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MeetingSettings {
    /// Record what the computer plays along with the microphone.
    pub capture_system_audio: bool,
    /// Where system audio comes from, as listed by `platform::system_audio_sources`,
    /// or empty for the default output.
    pub system_audio_source: String,
    /// Write notes with the cleanup model when a meeting ends.
    pub summarize_automatically: bool,
}

impl Default for MeetingSettings {
    fn default() -> Self {
        Self {
            capture_system_audio: true,
            system_audio_source: String::new(),
            summarize_automatically: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Global shortcut in Tauri accelerator syntax, e.g. "Ctrl+Shift+Space".
    pub hotkey: String,
    pub recording_mode: RecordingMode,
    pub live_typing: bool,
    /// Id from `models::CATALOG`, or empty when nothing is selected yet.
    pub selected_model: String,
    /// Whisper language code, or "auto" to detect.
    pub language: String,
    /// Most recently chosen languages, newest first, at most five, never "auto".
    pub recent_languages: Vec<String>,
    /// Input device id from cpal, or empty for the system default.
    pub input_device: String,
    pub auto_edit: bool,
    pub smart_trailing_punctuation: bool,
    pub cleanup: CleanupSettings,
    pub restore_clipboard: bool,
    /// Keep each dictation's audio next to its transcript, so it can be replayed.
    pub save_recordings: bool,
    /// Add misheard words to the dictionary when the user corrects a transcript.
    pub learn_corrections: bool,
    pub always_show_pill: bool,
    pub pill_position: PillPosition,
    pub show_tray_icon: bool,
    pub auto_update: bool,
    pub dictionary: Vec<DictionaryEntry>,
    pub has_completed_onboarding: bool,
    /// Set once SpeakType 1's data has been brought over, so it only happens once.
    pub has_imported_v1: bool,
    /// Set once the app has sent a new user to AI Models, so it only happens once.
    pub has_shown_model_prompt: bool,
    pub meetings: MeetingSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            hotkey: crate::platform::DEFAULT_HOTKEY.to_string(),
            recording_mode: RecordingMode::Hold,
            live_typing: false,
            selected_model: String::new(),
            language: "auto".to_string(),
            recent_languages: Vec::new(),
            input_device: String::new(),
            auto_edit: false,
            smart_trailing_punctuation: true,
            cleanup: CleanupSettings::default(),
            restore_clipboard: true,
            save_recordings: true,
            learn_corrections: true,
            always_show_pill: false,
            pill_position: PillPosition::BottomCenter,
            show_tray_icon: true,
            auto_update: true,
            dictionary: Vec::new(),
            has_completed_onboarding: false,
            has_imported_v1: false,
            has_shown_model_prompt: false,
            meetings: MeetingSettings::default(),
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            path: config_dir.join("settings.json"),
        }
    }

    /// Whether settings have ever been saved here, i.e. this isn't a fresh install.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Reads settings, falling back to defaults if the file is missing or unreadable.
    ///
    /// An unreadable file is moved aside to `settings.json.corrupt` and logged,
    /// so the next save doesn't silently overwrite the user's settings.
    pub fn load(&self) -> Settings {
        let parsed = match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            Err(e) if e.kind() == ErrorKind::NotFound => return Settings::default(),
            Err(e) => Err(e.to_string()),
        };
        parsed.unwrap_or_else(|e| {
            let backup = self.path.with_extension("json.corrupt");
            eprintln!(
                "[settings] couldn't read {}: {e}; moving it to {}",
                self.path.display(),
                backup.display()
            );
            if let Err(e) = fs::rename(&self.path, &backup) {
                eprintln!("[settings] couldn't move it aside: {e}");
            }
            Settings::default()
        })
    }

    /// Saves settings. Writes to a temporary file, flushes it to disk and renames
    /// it into place, so a crash mid-write can't leave a truncated file.
    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        let result = write_synced(&tmp, &json).and_then(|()| fs::rename(&tmp, &self.path));
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(|e| e.to_string())
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (SettingsStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("speaktype-test-{}", uuid::Uuid::new_v4()));
        (SettingsStore::new(dir.join("config")), dir)
    }

    #[test]
    fn missing_file_loads_defaults() {
        let (store, dir) = temp_store();
        let settings = store.load();
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.language, "auto");
        assert_eq!(settings.pill_position, PillPosition::BottomCenter);
        assert!(settings.smart_trailing_punctuation && settings.restore_clipboard);
        assert!(!dir.exists(), "loading doesn't create anything");
    }

    #[test]
    fn save_and_load_round_trip() {
        let (store, dir) = temp_store();
        let mut settings = Settings {
            recording_mode: RecordingMode::Toggle,
            selected_model: "base".into(),
            recent_languages: vec!["de".into(), "fr".into()],
            ..Settings::default()
        };
        store.save(&settings).unwrap();
        settings.theme = Theme::Dark;
        store.save(&settings).unwrap();

        let loaded = store.load();
        assert_eq!(loaded.theme, Theme::Dark);
        assert_eq!(loaded.recording_mode, RecordingMode::Toggle);
        assert_eq!(loaded.selected_model, "base");
        assert_eq!(loaded.recent_languages, ["de", "fr"]);
        assert!(!dir.join("config/settings.json.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn older_files_missing_fields_keep_what_they_have() {
        let json = r#"{
            "theme": "light",
            "recordingMode": "toggle",
            "pillPosition": "topRight",
            "dictionary": [{"id": "1", "trigger": "speak type", "replacement": "SpeakType"}]
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.theme, Theme::Light);
        assert_eq!(settings.recording_mode, RecordingMode::Toggle);
        assert_eq!(settings.pill_position, PillPosition::TopRight);
        assert_eq!(settings.hotkey, crate::platform::DEFAULT_HOTKEY);
        assert!(settings.show_tray_icon);
        assert!(
            settings.save_recordings,
            "recordings stay on for people upgrading"
        );
        assert!(!settings.live_typing);
        let entry = &settings.dictionary[0];
        assert!(entry.is_enabled && entry.match_whole_word);
        assert_eq!(settings.cleanup, CleanupSettings::default());
    }

    #[test]
    fn partial_cleanup_settings_keep_defaults_for_the_rest() {
        let json = r#"{"cleanup": {"enabled": true, "backend": "server"}}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.cleanup.enabled);
        assert_eq!(settings.cleanup.backend, CleanupBackend::Server);
        assert_eq!(settings.cleanup.server_url, "http://localhost:11434/v1");
    }

    #[test]
    fn serializes_with_the_field_names_the_frontend_expects() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        for key in [
            "theme",
            "hotkey",
            "recordingMode",
            "liveTyping",
            "selectedModel",
            "language",
            "recentLanguages",
            "inputDevice",
            "autoEdit",
            "smartTrailingPunctuation",
            "cleanup",
            "restoreClipboard",
            "saveRecordings",
            "learnCorrections",
            "alwaysShowPill",
            "pillPosition",
            "showTrayIcon",
            "autoUpdate",
            "dictionary",
            "hasCompletedOnboarding",
            "hasShownModelPrompt",
            "meetings",
        ] {
            assert!(value.get(key).is_some(), "missing {key}");
        }
        assert_eq!(value["pillPosition"], "bottomCenter");
        assert_eq!(value["recordingMode"], "hold");
        assert_eq!(value["cleanup"]["backend"], "builtin");
        assert_eq!(value["cleanup"]["enabled"], false);
        assert!(value["cleanup"].get("serverUrl").is_some());
    }

    #[test]
    fn unreadable_file_is_set_aside_and_defaults_load() {
        let (store, dir) = temp_store();
        fs::create_dir_all(dir.join("config")).unwrap();
        fs::write(dir.join("config/settings.json"), r#"{"theme": "sepia"}"#).unwrap();

        assert_eq!(store.load().theme, Theme::System);
        assert!(dir.join("config/settings.json.corrupt").is_file());
        assert!(!dir.join("config/settings.json").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
