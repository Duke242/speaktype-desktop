//! Inserts text into the focused app by putting it on the clipboard and
//! simulating the paste shortcut, then puts the user's clipboard back.
//!
//! One thread owns the clipboard for the app's lifetime. On Linux the clipboard
//! is served by the process that set it, so a short-lived handle would lose the
//! text before the target app reads it.

use std::{
    borrow::Cow,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use arboard::{Clipboard, ImageData};
use enigo::{Enigo, Settings};

use crate::platform;

/// Gives the user time to let go of the hotkey's modifier keys, which would
/// otherwise combine with the simulated paste shortcut.
const BEFORE_PASTE: Duration = Duration::from_millis(250);
/// How long the target app gets to read the clipboard before it is restored.
const BEFORE_RESTORE: Duration = Duration::from_millis(350);

struct Job {
    text: String,
    restore_clipboard: bool,
}

#[derive(Clone)]
pub struct Paster {
    tx: Sender<Job>,
}

enum Saved {
    Text(String),
    Image(ImageData<'static>),
    Empty,
}

impl Paster {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("paste".into())
            .spawn(move || {
                let mut clipboard = match Clipboard::new() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[paste] clipboard unavailable: {e}");
                        return;
                    }
                };
                let mut enigo = None;
                for job in rx {
                    // Created on first use so the macOS permission prompt appears
                    // when the user first dictates, not at launch.
                    if enigo.is_none() {
                        enigo = Enigo::new(&Settings::default())
                            .map_err(|e| eprintln!("[paste] keyboard simulation unavailable: {e}"))
                            .ok();
                    }
                    if let Err(e) = paste(&mut clipboard, enigo.as_mut(), &job) {
                        eprintln!("[paste] {e}");
                    }
                }
            })
            .expect("failed to start paste thread");
        Self { tx }
    }

    pub fn paste(&self, text: String, restore_clipboard: bool) {
        let _ = self.tx.send(Job {
            text,
            restore_clipboard,
        });
    }
}

fn paste(clipboard: &mut Clipboard, enigo: Option<&mut Enigo>, job: &Job) -> Result<(), String> {
    let saved = job.restore_clipboard.then(|| save(clipboard));
    clipboard
        .set_text(job.text.as_str())
        .map_err(|e| format!("Couldn't set clipboard: {e}"))?;

    thread::sleep(BEFORE_PASTE);
    // If this fails the text stays on the clipboard for the user to paste by hand.
    platform::send_paste_shortcut(enigo)?;

    let Some(saved) = saved else {
        return Ok(());
    };
    thread::sleep(BEFORE_RESTORE);
    // If the user copied something else in the meantime, keep that instead.
    if clipboard.get_text().ok().as_deref() != Some(job.text.as_str()) {
        return Ok(());
    }
    match saved {
        Saved::Text(text) => clipboard.set_text(text),
        Saved::Image(image) => clipboard.set_image(image),
        Saved::Empty => clipboard.clear(),
    }
    .map_err(|e| format!("Couldn't restore clipboard: {e}"))
}

fn save(clipboard: &mut Clipboard) -> Saved {
    if let Ok(text) = clipboard.get_text() {
        return Saved::Text(text);
    }
    if let Ok(image) = clipboard.get_image() {
        return Saved::Image(ImageData {
            width: image.width,
            height: image.height,
            bytes: Cow::Owned(image.bytes.into_owned()),
        });
    }
    Saved::Empty
}
