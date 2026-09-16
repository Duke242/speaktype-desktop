//! The dictation state machine: hotkey, record, transcribe, paste.
//!
//! All events go through one controller thread, so the session state has a
//! single owner. This also keeps shortcut registration out of the global
//! shortcut plugin's event handler, which holds a lock while it runs.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use serde::Serialize;

use crate::{
    AppState,
    audio::{Captured, Recording},
    history::now_ms,
    paste::Paster,
    pill::{self, PillState},
    pipeline,
    platform::{self, HotkeyEvent},
    settings::RecordingMode,
};

const CANCEL_KEY: &str = "Escape";

/// Where a finished transcription goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Destination {
    /// Pasted into the focused app.
    Paste,
    /// Shown in the Transcribe Audio screen.
    Screen,
}

/// Broadcast to the UI as `dictation-state`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum DictationState {
    Idle,
    #[serde(rename_all = "camelCase")]
    Recording {
        started_at_ms: u64,
        destination: Destination,
    },
    Transcribing {
        destination: Destination,
    },
}

/// Broadcast to the UI as `dictation-result` when a transcription for the screen finishes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationResult {
    pub text: Option<String>,
    pub error: Option<String>,
}

pub enum Event {
    HotkeyDown,
    HotkeyUp,
    /// Another key was pressed while a single-modifier hotkey was held.
    HotkeyInterrupted,
    /// Start or stop from the UI or tray, regardless of recording mode.
    Toggle(Destination),
    Escape,
    Transcribed {
        session: u64,
        outcome: Outcome,
    },
    MessageExpired {
        generation: u64,
    },
    /// Settings changed: the pill may need to appear, hide or move.
    RefreshPill,
}

pub type Outcome = Result<String, pipeline::Error>;

enum Phase {
    Idle,
    Recording(Recording, Destination),
    Transcribing {
        session: u64,
        cancelled: Arc<AtomicBool>,
        destination: Destination,
    },
    /// A short status message before returning to idle.
    Message {
        generation: u64,
    },
}

#[derive(Clone)]
pub struct Controller {
    tx: Sender<Event>,
}

impl Controller {
    pub fn spawn(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let controller = Self { tx };
        let mut session = Session {
            app,
            controller: controller.clone(),
            paster: Paster::spawn(),
            phase: Phase::Idle,
            hotkey_down: false,
            next_id: 0,
        };
        thread::Builder::new()
            .name("dictation".into())
            .spawn(move || {
                for event in rx {
                    session.handle(event);
                }
            })
            .expect("failed to start dictation thread");
        controller
    }

    pub fn send(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    /// Registers the user's hotkey, replacing `previous` if given. Single
    /// modifier keys like "Fn" use the platform's own listener.
    pub fn register_hotkey(
        &self,
        app: &AppHandle,
        hotkey: &str,
        previous: Option<&str>,
    ) -> Result<(), String> {
        let shortcuts = app.global_shortcut();
        if let Some(previous) = previous {
            if platform::MODIFIER_HOTKEYS.contains(&previous) {
                platform::stop_modifier_hotkey();
            } else {
                let _ = shortcuts.unregister(previous);
            }
        }

        let controller = self.clone();
        if platform::MODIFIER_HOTKEYS.contains(&hotkey) {
            return platform::start_modifier_hotkey(
                hotkey,
                Box::new(move |event| {
                    controller.send(match event {
                        HotkeyEvent::Down => Event::HotkeyDown,
                        HotkeyEvent::Up => Event::HotkeyUp,
                        HotkeyEvent::Interrupted => Event::HotkeyInterrupted,
                    })
                }),
            );
        }
        shortcuts
            .on_shortcut(hotkey, move |_, _, event| {
                controller.send(match event.state {
                    ShortcutState::Pressed => Event::HotkeyDown,
                    ShortcutState::Released => Event::HotkeyUp,
                });
            })
            .map_err(|e| format!("Couldn't register {hotkey}: {e}"))
    }
}

struct Session {
    app: AppHandle,
    controller: Controller,
    paster: Paster,
    phase: Phase,
    /// Ignores key repeat while the hotkey is held.
    hotkey_down: bool,
    next_id: u64,
}

impl Session {
    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::HotkeyDown => {
                if self.hotkey_down {
                    return;
                }
                self.hotkey_down = true;
                let mode = self.state().settings().recording_mode;
                if mode == RecordingMode::Toggle && matches!(self.phase, Phase::Recording(..)) {
                    self.stop(false);
                } else {
                    self.start(Destination::Paste);
                }
            }
            Event::HotkeyUp => {
                if !self.hotkey_down {
                    return;
                }
                self.hotkey_down = false;
                let mode = self.state().settings().recording_mode;
                if mode == RecordingMode::Hold && matches!(self.phase, Phase::Recording(..)) {
                    self.stop(false);
                }
            }
            Event::HotkeyInterrupted => {
                self.hotkey_down = false;
                let mode = self.state().settings().recording_mode;
                if mode == RecordingMode::Hold && matches!(self.phase, Phase::Recording(..)) {
                    self.discard();
                }
            }
            Event::Toggle(destination) => {
                if matches!(self.phase, Phase::Recording(..)) {
                    self.stop(false);
                } else {
                    self.start(destination);
                }
            }
            Event::Escape => match &self.phase {
                // Still transcribed and saved to history, but not pasted.
                Phase::Recording(..) => self.stop(true),
                // The transcription finishes in the background and lands in history.
                Phase::Transcribing { cancelled, .. } => {
                    cancelled.store(true, Ordering::Relaxed);
                    self.flash("Stopping transcription...", 800);
                }
                _ => {}
            },
            Event::Transcribed { session, outcome } => {
                let Phase::Transcribing {
                    session: current,
                    cancelled,
                    destination,
                } = &self.phase
                else {
                    return;
                };
                if *current != session {
                    return;
                }
                let cancelled = cancelled.load(Ordering::Relaxed);
                let destination = *destination;

                if destination == Destination::Screen {
                    let result = match &outcome {
                        Ok(text) => DictationResult { text: Some(text.clone()), error: None },
                        Err(e) => DictationResult { text: None, error: Some(e.message().into()) },
                    };
                    let _ = self.app.emit("dictation-result", result);
                }
                match outcome {
                    Ok(text) => {
                        self.go_idle();
                        if !cancelled && destination == Destination::Paste {
                            let restore = self.state().settings().restore_clipboard;
                            self.paster.paste(text, restore);
                        }
                    }
                    Err(e) => {
                        if let Some(detail) = e.detail() {
                            eprintln!("[dictation] {detail}");
                        }
                        let millis = if matches!(e, pipeline::Error::NoSpeech) { 1500 } else { 2000 };
                        self.flash(e.message(), millis);
                    }
                }
            }
            Event::MessageExpired { generation } => {
                if matches!(self.phase, Phase::Message { generation: g } if g == generation) {
                    self.go_idle();
                }
            }
            Event::RefreshPill => {
                if matches!(self.phase, Phase::Idle) {
                    pill::place(&self.app, self.state().settings().pill_position);
                    self.go_idle();
                }
            }
        }
    }

    fn start(&mut self, destination: Destination) {
        if matches!(self.phase, Phase::Recording(..) | Phase::Transcribing { .. }) {
            return;
        }
        let settings = self.state().settings();
        if settings.selected_model.is_empty() {
            return self.flash("No model selected", 2000);
        }
        if !self.state().models.is_downloaded(&settings.selected_model) {
            return self.flash("Model not downloaded", 2000);
        }

        let app = self.app.clone();
        let recording = Recording::start(&settings.input_device, move |level| {
            let _ = app.emit_to(pill::LABEL, "pill-level", level);
        });
        match recording {
            Ok(recording) => {
                let started_at_ms = now_ms();
                self.phase = Phase::Recording(recording, destination);
                self.set_cancel_key(true);
                self.show(PillState::Recording { started_at_ms });
                self.broadcast(DictationState::Recording { started_at_ms, destination });
                // Load the model while the user speaks, so it's ready on release.
                self.state().warm_up(&self.app, &settings.selected_model);
            }
            Err(e) => {
                eprintln!("[dictation] {e}");
                self.flash("Microphone unavailable", 2000);
            }
        }
    }

    fn stop(&mut self, cancelled: bool) {
        let Phase::Recording(recording, destination) =
            std::mem::replace(&mut self.phase, Phase::Idle)
        else {
            return;
        };
        let captured = match recording.finish() {
            Ok(captured) => captured,
            Err(e) => {
                eprintln!("[dictation] {e}");
                return self.flash("Recording failed", 2000);
            }
        };

        self.next_id += 1;
        let session = self.next_id;
        let cancelled = Arc::new(AtomicBool::new(cancelled));
        self.phase = Phase::Transcribing {
            session,
            cancelled: cancelled.clone(),
            destination,
        };
        let message = if cancelled.load(Ordering::Relaxed) {
            "Stopping transcription..."
        } else {
            "Transcribing..."
        };
        self.show(PillState::Processing {
            message: message.into(),
        });
        self.broadcast(DictationState::Transcribing { destination });

        let app = self.app.clone();
        let controller = self.controller.clone();
        thread::spawn(move || {
            let outcome = transcribe(&app, captured, &cancelled);
            controller.send(Event::Transcribed { session, outcome });
        });
    }

    /// Stops recording and throws the audio away.
    fn discard(&mut self) {
        if let Phase::Recording(recording, _) = std::mem::replace(&mut self.phase, Phase::Idle) {
            let _ = recording.finish();
        }
        self.go_idle();
    }

    /// Shows a status message in the pill, then returns to idle.
    fn flash(&mut self, message: &str, millis: u64) {
        self.next_id += 1;
        let generation = self.next_id;
        self.phase = Phase::Message { generation };
        self.set_cancel_key(false);
        self.show(PillState::Processing {
            message: message.into(),
        });
        self.broadcast(DictationState::Idle);
        let controller = self.controller.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(millis));
            controller.send(Event::MessageExpired { generation });
        });
    }

    fn go_idle(&mut self) {
        self.phase = Phase::Idle;
        self.set_cancel_key(false);
        self.show(PillState::Idle);
        self.broadcast(DictationState::Idle);
    }

    fn show(&self, state: PillState) {
        let settings = self.state().settings();
        pill::update(
            &self.app,
            state,
            settings.pill_position,
            settings.always_show_pill,
        );
        // The pill takes clicks only while recording, so its controls work.
        pill::set_interactive(&self.app, matches!(self.phase, Phase::Recording(..)));
    }

    fn broadcast(&self, state: DictationState) {
        let _ = self.app.emit("dictation-state", &state);
        *self.state().dictation_state.lock().unwrap() = state;
    }

    /// Escape is only claimed while a dictation is active, so other apps keep it otherwise.
    fn set_cancel_key(&self, active: bool) {
        let shortcuts = self.app.global_shortcut();
        let registered = shortcuts.is_registered(CANCEL_KEY);
        if active && !registered {
            let controller = self.controller.clone();
            let result = shortcuts.on_shortcut(CANCEL_KEY, move |_, _, event| {
                if event.state == ShortcutState::Pressed {
                    controller.send(Event::Escape);
                }
            });
            if let Err(e) = result {
                eprintln!("[dictation] couldn't register Escape: {e}");
            }
        } else if !active && registered {
            let _ = shortcuts.unregister(CANCEL_KEY);
        }
    }
}

/// Runs on a worker thread.
fn transcribe(app: &AppHandle, captured: Captured, cancelled: &AtomicBool) -> Outcome {
    let settings = app.state::<AppState>().settings();
    // After Escape the pill has moved on, so this worker stops updating it.
    let on_warming = |warming: bool| {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        let state = if warming {
            PillState::Warming
        } else {
            PillState::Processing { message: "Transcribing...".into() }
        };
        pill::update(app, state, settings.pill_position, settings.always_show_pill);
    };
    pipeline::run(app, &captured.samples, captured.duration_secs, on_warming)
        .map(|item| item.transcript)
}
