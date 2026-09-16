//! Past dictations and usage stats, stored as JSON in the app's data directory.
//!
//! Stats are kept in their own file so clearing or deleting history doesn't
//! reset the Dashboard and Statistics numbers, matching the macOS app.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
    pub transcript: String,
    pub duration_secs: f64,
    pub model: String,
    pub word_count: usize,
    /// The recording this came from, if it was kept.
    #[serde(default)]
    pub audio_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsEntry {
    pub created_at: u64,
    pub word_count: usize,
    pub duration_secs: f64,
}

pub struct History {
    items_path: PathBuf,
    stats_path: PathBuf,
    recordings_dir: PathBuf,
    items: Vec<HistoryItem>,
    stats: Vec<StatsEntry>,
}

impl History {
    pub fn load(data_dir: &Path) -> Self {
        let items_path = data_dir.join("history.json");
        let stats_path = data_dir.join("stats.json");
        let items: Vec<HistoryItem> = read_json(&items_path);
        let mut stats: Vec<StatsEntry> = read_json(&stats_path);
        if stats.is_empty() && !items.is_empty() {
            stats = items.iter().map(stats_entry).collect();
        }
        Self {
            items_path,
            stats_path,
            recordings_dir: data_dir.join("recordings"),
            items,
            stats,
        }
    }

    pub fn recordings_dir(&self) -> &Path {
        &self.recordings_dir
    }

    pub fn items(&self) -> &[HistoryItem] {
        &self.items
    }

    pub fn stats(&self) -> &[StatsEntry] {
        &self.stats
    }

    pub fn add(
        &mut self,
        transcript: &str,
        duration_secs: f64,
        model: &str,
        audio_path: Option<PathBuf>,
    ) -> Result<Option<HistoryItem>, String> {
        if transcript.is_empty() {
            return Ok(None);
        }
        let item = HistoryItem {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: now_ms(),
            transcript: transcript.to_string(),
            duration_secs,
            model: model.to_string(),
            word_count: transcript.split_whitespace().count(),
            audio_path: audio_path.map(|p| p.to_string_lossy().into_owned()),
        };
        self.items.insert(0, item.clone());
        self.stats.insert(0, stats_entry(&item));
        write_json(&self.items_path, &self.items)?;
        write_json(&self.stats_path, &self.stats)?;
        Ok(Some(item))
    }

    /// Removes an item and its recording. Stats are kept.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        if let Some(item) = self.items.iter().find(|item| item.id == id) {
            remove_audio(item);
        }
        self.items.retain(|item| item.id != id);
        write_json(&self.items_path, &self.items)
    }

    /// Removes every item and recording. Stats are kept.
    pub fn clear(&mut self) -> Result<(), String> {
        self.items.iter().for_each(remove_audio);
        self.items.clear();
        write_json(&self.items_path, &self.items)
    }

    pub fn audio_path(&self, id: &str) -> Option<PathBuf> {
        self.items
            .iter()
            .find(|item| item.id == id)
            .and_then(|item| item.audio_path.as_ref())
            .map(PathBuf::from)
            .filter(|path| path.is_file())
    }
}

fn stats_entry(item: &HistoryItem) -> StatsEntry {
    StatsEntry {
        created_at: item.created_at,
        word_count: item.word_count,
        duration_secs: item.duration_secs,
    }
}

fn remove_audio(item: &HistoryItem) {
    if let Some(path) = &item.audio_path {
        let _ = fs::remove_file(path);
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes to a temporary file and renames it, so a crash can't truncate the file.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}
