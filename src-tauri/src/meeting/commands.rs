//! Commands the Meetings screen calls through `invoke`. Registered in `lib.rs`
//! next to the rest, in `commands.rs`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use super::{MeetingListItem, MeetingState, MeetingView, Meetings, capture::AudioSource};
use crate::{AppState, cleanup, platform};

type CommandResult<T> = Result<T, String>;

/// Runs blocking work, like opening audio devices or waiting for a model,
/// off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSetup {
    /// What to know about recording system audio on this OS.
    system_audio_note: Option<&'static str>,
    system_audio_sources: Vec<AudioSource>,
    /// Whether a model is set up to write notes.
    notes_available: bool,
}

#[tauri::command]
pub async fn get_meeting_setup(app: AppHandle) -> CommandResult<MeetingSetup> {
    blocking(move || {
        let state = app.state::<AppState>();
        Ok(MeetingSetup {
            system_audio_note: platform::SYSTEM_AUDIO_NOTE,
            system_audio_sources: platform::system_audio_sources(),
            notes_available: cleanup::is_set_up(&state.settings().cleanup, &state.models),
        })
    })
    .await
}

#[tauri::command]
pub async fn get_meeting_state(meetings: State<'_, Meetings>) -> CommandResult<MeetingState> {
    Ok(meetings.state())
}

#[tauri::command]
pub async fn start_meeting(app: AppHandle, title: String) -> CommandResult<MeetingState> {
    blocking(move || app.state::<Meetings>().start(&app, &title)).await
}

#[tauri::command]
pub async fn stop_meeting(meetings: State<'_, Meetings>) -> CommandResult<()> {
    meetings.stop();
    Ok(())
}

#[tauri::command]
pub async fn list_meetings(
    app: AppHandle,
    query: Option<String>,
) -> CommandResult<Vec<MeetingListItem>> {
    let query = query.unwrap_or_default();
    blocking(move || Ok(app.state::<Meetings>().store.list_items(&query))).await
}

#[tauri::command]
pub async fn get_meeting(meetings: State<'_, Meetings>, id: String) -> CommandResult<MeetingView> {
    meetings.get(&id)
}

#[tauri::command]
pub async fn rename_meeting(
    app: AppHandle,
    meetings: State<'_, Meetings>,
    id: String,
    title: String,
) -> CommandResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A meeting needs a title".into());
    }
    meetings.update(&app, &id, |meeting| meeting.title = title.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_meeting(
    app: AppHandle,
    meetings: State<'_, Meetings>,
    id: String,
) -> CommandResult<()> {
    meetings.delete(&app, &id)
}

/// Writes notes for a meeting. Resolves when they're saved, which can take a
/// few minutes for a long meeting on a small computer.
#[tauri::command]
pub async fn summarize_meeting(app: AppHandle, id: String) -> CommandResult<MeetingView> {
    blocking(move || app.state::<Meetings>().summarize(&app, &id)).await
}

fn audio_path(meetings: &Meetings, id: &str) -> CommandResult<PathBuf> {
    meetings
        .get(id)?
        .meeting
        .audio_path
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .ok_or_else(|| "The recording for this meeting is missing".into())
}

#[tauri::command]
pub async fn reveal_meeting_audio(
    app: AppHandle,
    meetings: State<'_, Meetings>,
    id: String,
) -> CommandResult<()> {
    let path = audio_path(&meetings, &id)?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

/// Saves a meeting's notes and transcript, as written by the UI, to the
/// Downloads folder, and shows the file. Returns where it was saved.
#[tauri::command]
pub async fn export_meeting_markdown(
    app: AppHandle,
    meetings: State<'_, Meetings>,
    id: String,
    markdown: String,
) -> CommandResult<String> {
    let title = meetings.get(&id)?.meeting.title;
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .map_err(|e| e.to_string())?;
    let path = free_path(&dir, &file_stem(&title));
    tokio::fs::write(&path, markdown)
        .await
        .map_err(|e| format!("Couldn't save the file: {e}"))?;
    let _ = app.opener().reveal_item_in_dir(&path);
    Ok(path.to_string_lossy().into_owned())
}

/// A title as a file name that's safe on every OS.
fn file_stem(title: &str) -> String {
    let stem: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | ',' | '(' | ')') {
                c
            } else {
                '-'
            }
        })
        .take(80)
        .collect();
    let stem = stem.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if stem.is_empty() {
        "Meeting".into()
    } else {
        stem.to_string()
    }
}

/// `dir/stem.md`, or `stem (2).md` and so on if that's taken.
fn free_path(dir: &Path, stem: &str) -> PathBuf {
    (1..)
        .map(|n| {
            let name = if n == 1 {
                format!("{stem}.md")
            } else {
                format!("{stem} ({n}).md")
            };
            dir.join(name)
        })
        .find(|path| !path.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}.md")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_become_safe_file_names() {
        assert_eq!(
            file_stem("Weekly sync: Q3/Q4 plans?"),
            "Weekly sync- Q3-Q4 plans-"
        );
        assert_eq!(file_stem("../../etc"), "-..-etc");
        assert_eq!(file_stem("  ...  "), "Meeting");
        assert_eq!(file_stem("Café ☕"), "Café -");
    }

    #[test]
    fn existing_files_are_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("speaktype-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_path(&dir, "Sync"), dir.join("Sync.md"));
        std::fs::write(dir.join("Sync.md"), "").unwrap();
        assert_eq!(free_path(&dir, "Sync"), dir.join("Sync (2).md"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
