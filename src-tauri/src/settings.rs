//! User settings, stored as JSON in the app's config directory.

use std::{fs, path::PathBuf};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Global shortcut in Tauri accelerator syntax, e.g. "Ctrl+Shift+Space".
    pub hotkey: String,
    pub recording_mode: RecordingMode,
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
    pub restore_clipboard: bool,
    pub always_show_pill: bool,
    pub pill_position: PillPosition,
    pub show_tray_icon: bool,
    pub auto_update: bool,
    pub dictionary: Vec<DictionaryEntry>,
    pub has_completed_onboarding: bool,
    /// Set once the app has sent a new user to AI Models, so it only happens once.
    pub has_shown_model_prompt: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            hotkey: crate::platform::DEFAULT_HOTKEY.to_string(),
            recording_mode: RecordingMode::Hold,
            selected_model: String::new(),
            language: "auto".to_string(),
            recent_languages: Vec::new(),
            input_device: String::new(),
            auto_edit: false,
            smart_trailing_punctuation: true,
            restore_clipboard: true,
            always_show_pill: false,
            pill_position: PillPosition::BottomCenter,
            show_tray_icon: true,
            auto_update: true,
            dictionary: Vec::new(),
            has_completed_onboarding: false,
            has_shown_model_prompt: false,
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

    /// Reads settings, falling back to defaults if the file is missing or unreadable.
    pub fn load(&self) -> Settings {
        fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
        // Write then rename so a crash mid-write can't leave a truncated file.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}
