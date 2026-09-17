//! Past dictations and usage stats, stored as JSON in the app's data directory.
//!
//! Stats are kept in their own file so clearing or deleting history doesn't
//! reset the Dashboard and Statistics numbers, matching the macOS app.
//!
//! Changes apply in memory first and are then saved. If saving fails the error
//! is returned, and the change is written with the next successful save.

use std::{
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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
    created_at: u64,
    word_count: usize,
    duration_secs: f64,
}

pub struct History {
    items_path: PathBuf,
    stats_path: PathBuf,
    recordings_dir: PathBuf,
    items: Vec<HistoryItem>,
    stats: Vec<StatsEntry>,
}

impl History {
    /// Reads history and stats from `data_dir`. Missing files start empty.
    pub fn load(data_dir: &Path) -> Self {
        let items_path = data_dir.join("history.json");
        let stats_path = data_dir.join("stats.json");
        let items: Vec<HistoryItem> = read_json(&items_path);
        let mut stats: Vec<StatsEntry> = read_json(&stats_path);
        // History saved before stats had their own file.
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

    /// Newest first.
    pub fn items(&self) -> &[HistoryItem] {
        &self.items
    }

    /// Newest first.
    pub fn stats(&self) -> &[StatsEntry] {
        &self.stats
    }

    /// Records a transcript. Empty transcripts aren't kept and return `None`.
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
        // Save both even if the first fails; they're independent files.
        let items = write_json(&self.items_path, &self.items);
        let stats = write_json(&self.stats_path, &self.stats);
        items.and(stats).map(|()| Some(item))
    }

    /// Removes an item and its recording. Stats are kept.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return Ok(());
        };
        remove_audio(&self.items.remove(index));
        write_json(&self.items_path, &self.items)
    }

    /// Removes every item and recording. Stats are kept.
    pub fn clear(&mut self) -> Result<(), String> {
        self.items.drain(..).for_each(|item| remove_audio(&item));
        write_json(&self.items_path, &self.items)
    }

    /// The item's recording, if it has one and it's still on disk.
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
    if let Some(path) = &item.audio_path
        && let Err(e) = fs::remove_file(path)
        && e.kind() != ErrorKind::NotFound
    {
        eprintln!("[history] couldn't remove recording {path}: {e}");
    }
}

/// Milliseconds since the Unix epoch, or 0 if the clock is set before 1970.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

/// Reads a JSON file, or the default when there isn't one yet.
///
/// A file that exists but can't be read is moved aside to `*.corrupt` and
/// logged, so the next save doesn't silently overwrite data that might still be
/// recovered by hand.
fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    let parsed = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
        Err(e) if e.kind() == ErrorKind::NotFound => return T::default(),
        Err(e) => Err(e.to_string()),
    };
    parsed.unwrap_or_else(|e| {
        let backup = path.with_extension("json.corrupt");
        eprintln!(
            "[history] couldn't read {}: {e}; moving it to {}",
            path.display(),
            backup.display()
        );
        if let Err(e) = fs::rename(path, &backup) {
            eprintln!("[history] couldn't move it aside: {e}");
        }
        T::default()
    })
}

/// Writes to a temporary file, flushes it to disk and renames it over `path`, so
/// a crash or power loss leaves either the old file or the new one, never a
/// truncated mix.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let result = write_synced(&tmp, &json).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| e.to_string())
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("speaktype-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn items_and_stats_survive_a_reload() {
        let dir = TempDir::new();
        let mut history = History::load(&dir.0);
        assert!(history.items().is_empty() && history.stats().is_empty());

        assert!(history.add("", 1.0, "Tiny", None).unwrap().is_none());
        let first = history
            .add("hello world", 1.5, "Tiny", None)
            .unwrap()
            .unwrap();
        let second = history
            .add("  three  short words ", 2.0, "Base", None)
            .unwrap()
            .unwrap();
        assert_eq!(second.word_count, 3);

        let reloaded = History::load(&dir.0);
        let ids: Vec<_> = reloaded.items().iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, [second.id.as_str(), first.id.as_str()]);
        assert_eq!(reloaded.items()[1].transcript, "hello world");
        assert_eq!(reloaded.stats().len(), 2);
        assert!(!dir.0.join("history.json.tmp").exists());
    }

    #[test]
    fn delete_and_clear_remove_recordings_but_keep_stats() {
        let dir = TempDir::new();
        let mut history = History::load(&dir.0);
        let audio = dir.0.join("recordings").join("a.wav");
        fs::create_dir_all(audio.parent().unwrap()).unwrap();
        fs::write(&audio, b"RIFF").unwrap();

        let kept = history.add("keep me", 1.0, "Tiny", None).unwrap().unwrap();
        let item = history
            .add("with audio", 1.0, "Tiny", Some(audio.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(history.audio_path(&item.id), Some(audio.clone()));
        assert_eq!(history.audio_path(&kept.id), None);

        history.delete("no such id").unwrap();
        history.delete(&item.id).unwrap();
        assert!(!audio.exists());
        assert_eq!(history.audio_path(&item.id), None);

        let reloaded = History::load(&dir.0);
        assert_eq!(reloaded.items().len(), 1);
        assert_eq!(reloaded.stats().len(), 2);

        history.clear().unwrap();
        let reloaded = History::load(&dir.0);
        assert!(reloaded.items().is_empty());
        assert_eq!(reloaded.stats().len(), 2);
    }

    #[test]
    fn stats_are_rebuilt_from_older_history_files() {
        let dir = TempDir::new();
        // An item saved before `audioPath` existed.
        fs::write(
            dir.0.join("history.json"),
            r#"[{"id":"a","createdAt":5,"transcript":"hi there","durationSecs":1.0,"model":"Tiny","wordCount":2}]"#,
        )
        .unwrap();
        let history = History::load(&dir.0);
        assert_eq!(history.items()[0].audio_path, None);
        let stats = serde_json::to_value(history.stats()).unwrap();
        assert_eq!(
            stats,
            serde_json::json!([{"createdAt": 5, "wordCount": 2, "durationSecs": 1.0}])
        );
    }

    #[test]
    fn unreadable_files_are_set_aside_instead_of_overwritten() {
        let dir = TempDir::new();
        let path = dir.0.join("history.json");
        fs::write(&path, "{ not json").unwrap();

        let mut history = History::load(&dir.0);
        assert!(history.items().is_empty());
        assert_eq!(
            fs::read_to_string(dir.0.join("history.json.corrupt")).unwrap(),
            "{ not json"
        );
        history.add("fresh start", 1.0, "Tiny", None).unwrap();
        assert_eq!(History::load(&dir.0).items().len(), 1);
    }
}
