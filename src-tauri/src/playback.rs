//! Pauses music and videos while the user dictates, and resumes them after.
//!
//! Talking to media players can take a moment (D-Bus, WinRT, AppleScript), so
//! it happens on its own thread, in the order dictation asked for it. Only
//! what was paused here is resumed: if nothing was playing, or the user
//! paused it themselves, nothing starts playing afterwards.

use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use crate::{
    audio::Monitor,
    platform::{self, PausedMedia},
};

/// How long sound keeps coming out of the speakers after a player is told to
/// pause: the player's own delay, then the output device's buffering.
const PAUSE_TAIL: Duration = Duration::from_millis(150);

enum Request {
    /// Pauses for the recording `Monitor` reads, which shouldn't keep what
    /// the microphone heard of the media.
    Pause(Monitor),
    Resume,
}

/// Handle to the media thread. The thread exits once every handle is gone.
#[derive(Clone)]
pub struct Playback {
    tx: Sender<Request>,
}

impl Playback {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = thread::Builder::new().name("media".into()).spawn(move || {
            let mut paused: Option<PausedMedia> = None;
            for request in rx {
                match request {
                    // Already paused for this dictation.
                    Request::Pause(_) if paused.is_some() => {}
                    Request::Pause(recording) => {
                        paused = platform::pause_media();
                        if paused.is_some() {
                            recording.silence_until(PAUSE_TAIL);
                        }
                    }
                    Request::Resume => {
                        if let Some(media) = paused.take() {
                            platform::resume_media(media);
                        }
                    }
                }
            }
        });
        if let Err(e) = spawned {
            eprintln!("[media] couldn't start: {e}");
        }
        Self { tx }
    }

    /// Pauses whatever is playing, unless this already paused something. The
    /// start of `recording`, which heard it playing, is silenced.
    pub fn pause(&self, recording: Monitor) {
        let _ = self.tx.send(Request::Pause(recording));
    }

    /// Resumes what `pause` paused, if anything. Safe to call any time.
    pub fn resume(&self) {
        let _ = self.tx.send(Request::Resume);
    }
}
