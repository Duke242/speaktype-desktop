//! Saved meetings, one folder each in the app's data directory, next to
//! history: `meetings/<id>/meeting.json` and the recording beside it.
//!
//! Files are read when needed rather than kept in memory, since a long
//! meeting's transcript is large and most screens only need the list.

use std::{
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use super::{Meeting, MeetingListItem, Source, Status};

const FILE: &str = "meeting.json";
/// The mixed recording, made when a meeting ends.
pub const AUDIO_FILE: &str = "audio.wav";

pub struct MeetingStore {
    dir: PathBuf,
}

impl MeetingStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The folder for one meeting's files.
    pub fn folder(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    /// Where a track is written while recording.
    pub fn track_path(&self, id: &str, source: Source) -> PathBuf {
        self.folder(id).join(source.file_name())
    }

    pub fn save(&self, meeting: &Meeting) -> Result<(), String> {
        valid_id(&meeting.id)?;
        let path = self.folder(&meeting.id).join(FILE);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec(meeting).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        let result = write_synced(&tmp, &json).and_then(|()| fs::rename(&tmp, &path));
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(|e| e.to_string())
    }

    pub fn load(&self, id: &str) -> Result<Meeting, String> {
        valid_id(id)?;
        let bytes = fs::read(self.folder(id).join(FILE)).map_err(|e| match e.kind() {
            ErrorKind::NotFound => "This meeting was deleted".to_string(),
            _ => e.to_string(),
        })?;
        serde_json::from_slice(&bytes).map_err(|e| format!("Couldn't read this meeting: {e}"))
    }

    /// Every saved meeting, newest first. Folders that can't be read are
    /// skipped and logged, and left on disk.
    pub fn list(&self) -> Vec<Meeting> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut meetings: Vec<Meeting> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let id = entry.file_name().to_string_lossy().into_owned();
                match self.load(&id) {
                    Ok(meeting) => Some(meeting),
                    Err(e) => {
                        eprintln!("[meetings] skipping {id}: {e}");
                        None
                    }
                }
            })
            .collect();
        meetings.sort_by_key(|m| std::cmp::Reverse(m.created_at));
        meetings
    }

    /// Meetings whose title, transcript or notes contain `query`, newest first.
    pub fn list_items(&self, query: &str) -> Vec<MeetingListItem> {
        self.list()
            .iter()
            .filter(|m| m.matches(query))
            .map(MeetingListItem::from)
            .collect()
    }

    /// Removes a meeting and its recordings.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        valid_id(id)?;
        match fs::remove_dir_all(self.folder(id)) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    /// Finishes meetings left recording when the app quit or crashed: what was
    /// transcribed stays, and the microphone track becomes the recording.
    /// Call before any meeting starts. Returns how many were recovered.
    pub fn recover(&self) -> usize {
        let mut recovered = 0;
        for mut meeting in self.list() {
            if meeting.status == Status::Done {
                continue;
            }
            let folder = self.folder(&meeting.id);
            let audio = [AUDIO_FILE, Source::Microphone.file_name()]
                .iter()
                .map(|name| folder.join(name))
                .find(|path| path.is_file());
            meeting.audio_path = audio.map(|p| p.to_string_lossy().into_owned());
            meeting.duration_secs = meeting.duration_secs.max(
                meeting
                    .segments
                    .iter()
                    .map(|s| s.end_secs)
                    .fold(0.0, f64::max),
            );
            meeting.status = Status::Done;
            match self.save(&meeting) {
                Ok(()) => recovered += 1,
                Err(e) => eprintln!("[meetings] couldn't recover {}: {e}", meeting.id),
            }
        }
        recovered
    }
}

/// Ids are folder names, so they must not reach outside the meetings folder.
fn valid_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if ok {
        Ok(())
    } else {
        Err("Unknown meeting".into())
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
    use crate::meeting::Segment;

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

    fn meeting(created_at: u64, title: &str) -> Meeting {
        let mut meeting = Meeting::new(title, created_at, vec![Source::Microphone, Source::System]);
        meeting.status = Status::Done;
        meeting
    }

    #[test]
    fn meetings_survive_a_reload_newest_first() {
        let dir = TempDir::new();
        let store = MeetingStore::new(dir.0.join("meetings"));
        assert!(store.list().is_empty(), "no folder yet");

        let mut older = meeting(1_000, "Standup");
        older.insert_segment(Segment {
            start_secs: 10.0,
            end_secs: 14.0,
            source: Source::System,
            text: "Morning all.".into(),
        });
        older.insert_segment(Segment {
            start_secs: 2.0,
            end_secs: 6.0,
            source: Source::Microphone,
            text: "Hi.".into(),
        });
        store.save(&older).unwrap();
        store.save(&meeting(2_000, "Planning")).unwrap();

        let list = store.list();
        assert_eq!(
            list.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
            ["Planning", "Standup"]
        );
        let loaded = store.load(&older.id).unwrap();
        assert_eq!(loaded.segments.len(), 2);
        assert_eq!(loaded.segments[0].text, "Hi.", "kept in time order");
        assert_eq!(loaded.sources, [Source::Microphone, Source::System]);
        assert!(!store.folder(&older.id).join("meeting.json.tmp").exists());

        let items = store.list_items("");
        assert_eq!(store.list_items("morning").len(), 1);
        assert_eq!(items[1].word_count, 3);
        assert_eq!(items[1].preview, "Hi. Morning all.");
    }

    #[test]
    fn delete_removes_the_folder_and_recordings() {
        let dir = TempDir::new();
        let store = MeetingStore::new(dir.0.clone());
        let saved = meeting(1, "Retro");
        store.save(&saved).unwrap();
        fs::write(store.folder(&saved.id).join(AUDIO_FILE), b"RIFF").unwrap();

        store.delete(&saved.id).unwrap();
        assert!(!store.folder(&saved.id).exists());
        assert!(store.load(&saved.id).is_err());
        store.delete(&saved.id).unwrap();
    }

    #[test]
    fn ids_cannot_reach_outside_the_folder() {
        let dir = TempDir::new();
        let store = MeetingStore::new(dir.0.join("meetings"));
        fs::write(dir.0.join("meeting.json"), "{}").unwrap();
        for id in ["..", "../x", "", "a/b", "a\\b"] {
            assert!(store.load(id).is_err(), "{id}");
            assert!(store.delete(id).is_err(), "{id}");
        }
        assert!(dir.0.join("meeting.json").exists());
    }

    #[test]
    fn unreadable_meetings_are_skipped_not_deleted() {
        let dir = TempDir::new();
        let store = MeetingStore::new(dir.0.clone());
        store.save(&meeting(1, "Fine")).unwrap();
        let broken = dir.0.join("broken-1");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join(FILE), "{ not json").unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(broken.join(FILE).exists());
    }

    #[test]
    fn interrupted_recordings_are_finished_on_launch() {
        let dir = TempDir::new();
        let store = MeetingStore::new(dir.0.clone());
        let mut interrupted = meeting(1, "Call");
        interrupted.status = Status::Recording;
        interrupted.insert_segment(Segment {
            start_secs: 0.0,
            end_secs: 42.5,
            source: Source::Microphone,
            text: "So where were we".into(),
        });
        store.save(&interrupted).unwrap();
        fs::write(
            store.track_path(&interrupted.id, Source::Microphone),
            b"RIFF",
        )
        .unwrap();
        store.save(&meeting(2, "Done already")).unwrap();

        assert_eq!(store.recover(), 1);
        let loaded = store.load(&interrupted.id).unwrap();
        assert_eq!(loaded.status, Status::Done);
        assert_eq!(loaded.duration_secs, 42.5);
        assert!(
            loaded
                .audio_path
                .unwrap()
                .ends_with(Source::Microphone.file_name())
        );
        assert_eq!(store.recover(), 0);
    }

    #[test]
    fn older_files_missing_fields_still_load() {
        let dir = TempDir::new();
        let store = MeetingStore::new(dir.0.clone());
        fs::create_dir_all(dir.0.join("abc")).unwrap();
        fs::write(
            dir.0.join("abc").join(FILE),
            r#"{"id":"abc","title":"Old","createdAt":5}"#,
        )
        .unwrap();
        let loaded = store.load("abc").unwrap();
        assert_eq!(loaded.status, Status::Done);
        assert!(loaded.segments.is_empty() && loaded.notes.is_none());
    }
}
