//! Meeting notes: records the microphone and what the computer plays, shows a
//! transcript that grows during the meeting, and saves it with the recording.
//! Afterwards, the cleanup model can write a summary, action items and open
//! questions.
//!
//! - `capture` opens audio sources. System audio is OS-specific, in
//!   `platform::<os>::system_audio`.
//! - `mixer` keeps the two tracks in step and mixes them for playback.
//! - `segment` cuts each track into 15–30 second windows at pauses.
//! - `session` runs a recording: one thread writes each track to a WAV file,
//!   another reads windows back and transcribes them, so memory stays flat
//!   however long the meeting is.
//! - `summary` writes the notes, in parts for long meetings.
//! - `store` saves meetings next to history.
//!
//! The microphone is labelled "You" and system audio "Others": a cheap
//! speaker split that needs no diarization model.

pub mod capture;
pub mod commands;
mod mixer;
mod segment;
mod session;
mod store;
mod summary;

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, LockExt, cleanup};
pub use store::MeetingStore;
pub use summary::Notes;

/// Where a segment's audio came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The microphone: "You".
    Microphone,
    /// What the computer played: "Others".
    System,
}

impl Source {
    /// The track's file while recording.
    pub fn file_name(self) -> &'static str {
        match self {
            Source::Microphone => "microphone.wav",
            Source::System => "system.wav",
        }
    }
}

/// A stretch of transcript. Times are seconds from the meeting's start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start_secs: f64,
    pub end_secs: f64,
    pub source: Source,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Recording,
    /// Stopped; the last of the audio is being transcribed and saved.
    Processing,
    #[default]
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
    #[serde(default)]
    pub duration_secs: f64,
    #[serde(default)]
    pub status: Status,
    /// The tracks recorded. With both, segments are labelled "You" and "Others".
    #[serde(default)]
    pub sources: Vec<Source>,
    /// In time order.
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// The mixed recording, once the meeting has ended.
    #[serde(default)]
    pub audio_path: Option<String>,
    /// The speech model's name.
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub notes: Option<Notes>,
    /// Why the last attempt at notes failed.
    #[serde(default)]
    pub notes_error: Option<String>,
}

impl Meeting {
    pub fn new(title: &str, created_at: u64, sources: Vec<Source>) -> Self {
        let title = title.trim();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: if title.is_empty() {
                "Meeting".into()
            } else {
                title.into()
            },
            created_at,
            duration_secs: 0.0,
            status: Status::Recording,
            sources,
            segments: Vec::new(),
            audio_path: None,
            model: String::new(),
            notes: None,
            notes_error: None,
        }
    }

    /// Adds a segment in time order. The two tracks are transcribed in turn,
    /// so a segment can start before the last one added.
    pub fn insert_segment(&mut self, segment: Segment) {
        let at = self
            .segments
            .partition_point(|s| s.start_secs <= segment.start_secs);
        self.segments.insert(at, segment);
    }

    pub fn labelled(&self) -> bool {
        self.sources.contains(&Source::System) && self.sources.contains(&Source::Microphone)
    }

    /// Whether the title, transcript or notes contain `query`, ignoring case.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let has = |text: &str| text.to_lowercase().contains(&query);
        has(&self.title)
            || self.segments.iter().any(|s| has(&s.text))
            || self.notes.as_ref().is_some_and(|n| {
                has(&n.summary)
                    || n.action_items.iter().any(|i| has(i))
                    || n.open_questions.iter().any(|q| has(q))
            })
    }

    pub fn word_count(&self) -> usize {
        self.segments
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum()
    }
}

/// A row in the Meetings list, without the full transcript.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingListItem {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub duration_secs: f64,
    pub status: Status,
    pub word_count: usize,
    /// The start of the transcript.
    pub preview: String,
    pub has_notes: bool,
}

const PREVIEW_CHARS: usize = 220;

impl From<&Meeting> for MeetingListItem {
    fn from(meeting: &Meeting) -> Self {
        let transcript = meeting
            .segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        Self {
            id: meeting.id.clone(),
            title: meeting.title.clone(),
            created_at: meeting.created_at,
            duration_secs: meeting.duration_secs,
            status: meeting.status,
            word_count: meeting.word_count(),
            preview: transcript.chars().take(PREVIEW_CHARS).collect(),
            has_notes: meeting.notes.is_some(),
        }
    }
}

/// Broadcast to the UI as `meeting-state`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum MeetingState {
    Idle,
    #[serde(rename_all = "camelCase")]
    Recording {
        id: String,
        title: String,
        started_at_ms: u64,
        /// Whether what the computer plays is being recorded too.
        system_audio: bool,
        /// Why system audio couldn't be recorded, when it was asked for.
        system_error: Option<String>,
    },
    /// Transcribing the last of the audio and saving the recording.
    #[serde(rename_all = "camelCase")]
    Finishing {
        id: String,
    },
}

/// A meeting as the UI shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingView {
    #[serde(flatten)]
    pub meeting: Meeting,
    /// Notes are being written right now.
    pub summarizing: bool,
}

/// Everything about meetings that lives as long as the app.
pub struct Meetings {
    pub store: MeetingStore,
    state: Mutex<MeetingState>,
    /// The meeting being recorded or finished, shared with its threads.
    active: Mutex<Option<session::Session>>,
    summarizing: Mutex<HashSet<String>>,
}

impl Meetings {
    pub fn new(dir: PathBuf) -> Self {
        let store = MeetingStore::new(dir);
        let recovered = store.recover();
        if recovered > 0 {
            eprintln!("[meetings] finished {recovered} meeting(s) interrupted by a quit or crash");
        }
        Self {
            store,
            state: Mutex::new(MeetingState::Idle),
            active: Mutex::new(None),
            summarizing: Mutex::default(),
        }
    }

    pub fn state(&self) -> MeetingState {
        self.state.lock_unpoisoned().clone()
    }

    fn set_state(&self, app: &AppHandle, state: MeetingState) {
        *self.state.lock_unpoisoned() = state.clone();
        let _ = app.emit("meeting-state", state);
    }

    /// The meeting, from memory while it's being recorded.
    pub fn get(&self, id: &str) -> Result<MeetingView, String> {
        let live = self
            .active
            .lock_unpoisoned()
            .as_ref()
            .filter(|s| s.id == id)
            .map(|s| s.meeting.lock_unpoisoned().clone());
        let meeting = match live {
            Some(meeting) => meeting,
            None => self.store.load(id)?,
        };
        Ok(MeetingView {
            summarizing: self.summarizing.lock_unpoisoned().contains(id),
            meeting,
        })
    }

    /// Changes a meeting, in memory too if it's the one being recorded, and saves it.
    pub fn update(
        &self,
        app: &AppHandle,
        id: &str,
        change: impl FnOnce(&mut Meeting),
    ) -> Result<Meeting, String> {
        let active = self.active.lock_unpoisoned();
        let meeting = match active.as_ref().filter(|s| s.id == id) {
            Some(session) => {
                let mut meeting = session.meeting.lock_unpoisoned();
                change(&mut meeting);
                self.store.save(&meeting)?;
                meeting.clone()
            }
            None => {
                let mut meeting = self.store.load(id)?;
                change(&mut meeting);
                self.store.save(&meeting)?;
                meeting
            }
        };
        drop(active);
        let _ = app.emit("meetings-changed", ());
        Ok(meeting)
    }

    pub fn delete(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        if self
            .active
            .lock_unpoisoned()
            .as_ref()
            .is_some_and(|s| s.id == id)
        {
            return Err("Stop the meeting before deleting it".into());
        }
        self.store.delete(id)?;
        let _ = app.emit("meetings-changed", ());
        Ok(())
    }

    /// Starts recording a meeting with the current settings.
    pub fn start(&self, app: &AppHandle, title: &str) -> Result<MeetingState, String> {
        let mut active = self.active.lock_unpoisoned();
        if active.is_some() {
            return Err(match self.state() {
                MeetingState::Finishing { .. } => "The last meeting is still being saved".into(),
                _ => "A meeting is already being recorded".into(),
            });
        }
        let (session, state) = session::Session::start(app, &self.store, title)?;
        *active = Some(session);
        // Still holding `active`: a recording that ends at once reports Idle
        // from `finished`, which waits for the lock, so it can't be
        // overwritten by Recording here.
        self.set_state(app, state.clone());
        drop(active);
        let _ = app.emit("meetings-changed", ());
        Ok(state)
    }

    /// Stops recording. The rest is transcribed and saved in the background,
    /// then notes are written if that's turned on.
    pub fn stop(&self) {
        // The session reports Finishing itself once the microphone is closed.
        if let Some(session) = self.active.lock_unpoisoned().as_ref() {
            session.stop();
        }
    }

    /// Called by the session once everything is saved.
    fn finished(&self, app: &AppHandle, id: &str) {
        let mut active = self.active.lock_unpoisoned();
        if active.as_ref().is_some_and(|s| s.id == id) {
            *active = None;
        }
        drop(active);
        self.set_state(app, MeetingState::Idle);
        let _ = app.emit("meetings-changed", ());
        let settings = app.state::<AppState>().settings();
        let models = &app.state::<AppState>().models;
        if settings.meetings.summarize_automatically
            && cleanup::is_set_up(&settings.cleanup, models)
            && let Err(e) = self.summarize(app, id)
        {
            eprintln!("[meetings] couldn't write notes: {e}");
        }
    }

    /// Writes notes for a meeting with the cleanup model, and saves them.
    /// Blocks for as long as the model takes, so call it off the async runtime.
    pub fn summarize(&self, app: &AppHandle, id: &str) -> Result<MeetingView, String> {
        let state = app.state::<AppState>();
        let settings = state.settings();
        if !cleanup::is_set_up(&settings.cleanup, &state.models) {
            return Err(NOT_SET_UP.into());
        }
        let meeting = self.get(id)?.meeting;
        if !self.summarizing.lock_unpoisoned().insert(id.to_string()) {
            return Err("Notes for this meeting are already being written".into());
        }
        let _ = app.emit("meetings-changed", ());

        let lines = summary::transcript_lines(&meeting.segments, meeting.labelled());
        let started = std::time::Instant::now();
        let result = summary::summarize(&lines, |messages| {
            let prompt = cleanup::Prompt {
                messages,
                max_tokens: summary::MAX_TOKENS,
                timeout: NOTES_TIMEOUT,
            };
            state
                .cleaner
                .complete(&settings.cleanup, &state.models, &prompt)
        });
        eprintln!(
            "[meetings] notes for {} words in {:.1?}",
            meeting.word_count(),
            started.elapsed()
        );
        let saved = self.update(app, id, |meeting| match &result {
            Ok(notes) => {
                meeting.notes = Some(notes.clone());
                meeting.notes_error = None;
            }
            Err(e) => meeting.notes_error = Some(e.clone()),
        });
        self.summarizing.lock_unpoisoned().remove(id);
        let _ = app.emit("meetings-changed", ());
        result?;
        Ok(MeetingView {
            meeting: saved?,
            summarizing: false,
        })
    }
}

/// Shown when there's no model to write notes with.
pub const NOT_SET_UP: &str =
    "Set up AI cleanup in Settings to write meeting notes with a local model";

/// Each request to the model: a part of a long meeting, or merging parts.
const NOTES_TIMEOUT: Duration = Duration::from_secs(300);

/// Shares a meeting with the session's threads.
type Shared = Arc<Mutex<Meeting>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(start: f64, text: &str) -> Segment {
        Segment {
            start_secs: start,
            end_secs: start + 1.0,
            source: Source::Microphone,
            text: text.into(),
        }
    }

    #[test]
    fn segments_stay_in_time_order() {
        let mut meeting = Meeting::new("  ", 0, vec![Source::Microphone]);
        assert_eq!(meeting.title, "Meeting");
        for (start, text) in [(30.0, "c"), (0.0, "a"), (15.0, "b"), (30.0, "d")] {
            meeting.insert_segment(segment(start, text));
        }
        let texts: Vec<_> = meeting.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["a", "b", "c", "d"]);
        assert!(!meeting.labelled());
    }

    #[test]
    fn list_items_preview_and_meetings_search_everything() {
        let mut meeting = Meeting::new("Sync", 0, vec![Source::Microphone, Source::System]);
        meeting.insert_segment(segment(0.0, " Hello THERE "));
        meeting.insert_segment(segment(1.0, ""));
        meeting.insert_segment(segment(2.0, "General Kenobi"));
        let item = MeetingListItem::from(&meeting);
        assert_eq!(item.preview, "Hello THERE General Kenobi");
        assert!(meeting.matches("  general KEN "));
        assert!(meeting.matches("sync") && meeting.matches(""));
        assert!(!meeting.matches("obi-wan"));
        meeting.notes = Some(Notes {
            action_items: vec!["Find Obi-Wan".into()],
            ..Notes::default()
        });
        assert!(meeting.matches("obi-wan"));
        assert_eq!(item.word_count, 4);
        assert!(!item.has_notes);
        assert!(meeting.labelled());
    }

    #[test]
    fn state_serializes_for_the_ui() {
        let state = MeetingState::Recording {
            id: "a".into(),
            title: "Sync".into(),
            started_at_ms: 5,
            system_audio: true,
            system_error: None,
        };
        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!({"phase": "recording", "id": "a", "title": "Sync", "startedAtMs": 5, "systemAudio": true, "systemError": null})
        );
        assert_eq!(
            serde_json::to_value(MeetingState::Idle).unwrap(),
            serde_json::json!({"phase": "idle"})
        );
    }
}
