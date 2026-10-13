//! Pausing media. MediaRemote, the private framework behind Now Playing in
//! Control Center, reaches browsers and most players. macOS 15.4 and later no
//! longer tell other apps whether anything is playing, so Music and Spotify are
//! also asked directly, through AppleScript.

use std::{
    ffi::c_void,
    process::Command,
    sync::{OnceLock, mpsc},
    time::Duration,
};

use block2::{Block, RcBlock};
use core_foundation::{base::Boolean, bundle::CFBundle, string::CFString, url::CFURL};

const MEDIA_REMOTE: &str = "/System/Library/PrivateFrameworks/MediaRemote.framework";
/// `MRMediaRemoteCommand` values.
const PLAY: u32 = 0;
const PAUSE: u32 = 1;
/// How long MediaRemote gets to say whether something is playing.
const REPLY_TIMEOUT: Duration = Duration::from_millis(500);
/// Players asked through AppleScript, by bundle id.
const PLAYERS: &[&str] = &["com.apple.Music", "com.spotify.client"];

pub enum PausedMedia {
    /// Whatever Now Playing showed.
    NowPlaying,
    /// These players, by bundle id.
    Players(Vec<&'static str>),
}

/// Pauses what's playing.
pub fn pause_media() -> Option<PausedMedia> {
    if let Some(remote) = media_remote()
        && remote.is_playing() == Some(true)
        && remote.send(PAUSE)
    {
        return Some(PausedMedia::NowPlaying);
    }
    let paused: Vec<&'static str> = PLAYERS
        .iter()
        .copied()
        .filter(|&player| {
            is_running(player)
                && player_state(player).as_deref() == Some("playing")
                && tell(player, "pause").is_some()
        })
        .collect();
    (!paused.is_empty()).then_some(PausedMedia::Players(paused))
}

/// Plays what `pause_media` paused, unless the user has since started
/// something else, or stopped, quit or restarted the player.
pub fn resume_media(paused: PausedMedia) {
    match paused {
        PausedMedia::NowPlaying => {
            if let Some(remote) = media_remote()
                && remote.is_playing() == Some(false)
            {
                remote.send(PLAY);
            }
        }
        PausedMedia::Players(players) => {
            for player in players {
                if is_running(player) && player_state(player).as_deref() == Some("paused") {
                    tell(player, "play");
                }
            }
        }
    }
}

// Signatures from MediaRemote's private headers, as used by other Now Playing tools.
type IsPlayingFn = unsafe extern "C" fn(queue: *mut c_void, completion: &Block<dyn Fn(Boolean)>);
type SendCommandFn = unsafe extern "C" fn(command: u32, options: *const c_void) -> Boolean;

// libdispatch is part of libSystem, which every program links.
unsafe extern "C" {
    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
}

struct MediaRemote {
    is_playing: IsPlayingFn,
    send_command: SendCommandFn,
}

/// Loaded on first use. `None` if the framework or its functions are gone.
fn media_remote() -> Option<&'static MediaRemote> {
    static LOADED: OnceLock<Option<MediaRemote>> = OnceLock::new();
    LOADED
        .get_or_init(|| {
            let bundle = CFBundle::new(CFURL::from_path(MEDIA_REMOTE, true)?)?;
            let is_playing = bundle.function_pointer_for_name(CFString::new(
                "MRMediaRemoteGetNowPlayingApplicationIsPlaying",
            ));
            let send_command =
                bundle.function_pointer_for_name(CFString::new("MRMediaRemoteSendCommand"));
            // The function pointers point into it, so it stays loaded for good.
            std::mem::forget(bundle);
            if is_playing.is_null() || send_command.is_null() {
                eprintln!("[media] MediaRemote is unavailable");
                return None;
            }
            // SAFETY: both pointers are non-null functions exported by
            // MediaRemote, with the signatures declared above.
            let remote = unsafe {
                MediaRemote {
                    is_playing: std::mem::transmute::<*const c_void, IsPlayingFn>(is_playing),
                    send_command: std::mem::transmute::<*const c_void, SendCommandFn>(send_command),
                }
            };
            Some(remote)
        })
        .as_ref()
}

impl MediaRemote {
    /// `None` when MediaRemote doesn't answer in time.
    fn is_playing(&self) -> Option<bool> {
        let (tx, rx) = mpsc::channel();
        let completion = RcBlock::new(move |playing: Boolean| {
            let _ = tx.send(playing != 0);
        });
        // SAFETY: global queues are never released. MediaRemote copies the
        // block and calls it on that queue; it only sends on a channel.
        unsafe {
            (self.is_playing)(dispatch_get_global_queue(0, 0), &completion);
        }
        rx.recv_timeout(REPLY_TIMEOUT).ok()
    }

    fn send(&self, command: u32) -> bool {
        // SAFETY: a play or pause command takes no options.
        unsafe { (self.send_command)(command, std::ptr::null()) != 0 }
    }
}

/// Asking whether an app is running doesn't launch it, and doesn't need the
/// app's own scripting terms, so this works whether or not it is installed.
fn is_running(player: &str) -> bool {
    osascript(&format!("application id \"{player}\" is running")).as_deref() == Some("true")
}

/// "playing", "paused" or "stopped". Only ask a running player: telling one
/// that isn't would launch it.
fn player_state(player: &str) -> Option<String> {
    tell(player, "get player state as string")
}

fn tell(player: &str, command: &str) -> Option<String> {
    osascript(&format!("tell application id \"{player}\" to {command}"))
}

/// Runs AppleScript, returning its result. The first time it controls a
/// player, macOS asks the user to allow it.
fn osascript(script: &str) -> Option<String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
