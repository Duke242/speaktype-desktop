//! Pausing media through the system media controls (the ones above the volume
//! flyout), which browsers, Spotify, Media Player and most other apps use.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};

/// The sessions that were playing and got paused.
pub struct PausedMedia(Vec<Session>);

/// Pauses every session that is playing. Runs on the media thread, which
/// windows-rs joins to the multithreaded apartment on first use, so blocking
/// on the async calls is fine.
pub fn pause_media() -> Option<PausedMedia> {
    let sessions = Manager::RequestAsync()
        .and_then(|request| request.join())
        .and_then(|manager| manager.GetSessions())
        .map_err(|e| eprintln!("[media] couldn't list players: {e}"))
        .ok()?;
    let paused: Vec<Session> = sessions
        .into_iter()
        .filter(|session| {
            status(session) == Some(Status::Playing)
                && session
                    .TryPauseAsync()
                    .and_then(|pause| pause.join())
                    .unwrap_or(false)
        })
        .collect();
    (!paused.is_empty()).then_some(PausedMedia(paused))
}

/// Plays the sessions `pause_media` paused, unless the user has since stopped
/// or restarted them. Sessions that have closed fail quietly.
pub fn resume_media(paused: PausedMedia) {
    for session in paused.0 {
        if status(&session) == Some(Status::Paused) {
            let _ = session.TryPlayAsync().and_then(|play| play.join());
        }
    }
}

fn status(session: &Session) -> Option<Status> {
    session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .ok()
}
