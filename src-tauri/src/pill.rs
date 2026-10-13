//! The floating recorder pill: a small transparent window that never takes focus.
//!
//! The window is larger than the pill drawn in it, and transparent around it,
//! so it passes clicks through to what's underneath. While recording it takes
//! clicks, for its controls. An idle pill that stays on screen takes them only
//! while the cursor is over it, so it can be right-clicked to switch models.

use std::{
    sync::{Mutex, MutexGuard, Once},
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::{LockExt, settings::PillPosition};

pub const LABEL: &str = "pill";

/// Gap between the pill window and the screen edge, in logical pixels.
const INSET: f64 = 8.0;

/// The idle pill as Pill.tsx draws it, centered in the window, in logical
/// pixels, with a little room around it.
const IDLE_PILL: (f64, f64) = (58.0 + 8.0, 24.0 + 8.0);
/// How often the cursor is checked while an idle pill is on screen.
const HOVER_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clicks {
    /// Passed through to whatever is underneath.
    Never,
    Always,
    /// Only while the cursor is over the idle pill.
    OverIdlePill,
}

struct ClickState {
    clicks: Clicks,
    /// The idle pill is on screen, because it's set to always show.
    idle_shown: bool,
    /// What the window was last set to, if known.
    takes_clicks: Option<bool>,
}

static CLICK_STATE: Mutex<ClickState> = Mutex::new(ClickState {
    clicks: Clicks::Never,
    idle_shown: false,
    takes_clicks: None,
});

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum PillState {
    Idle,
    #[serde(rename_all = "camelCase")]
    Recording {
        started_at_ms: u64,
        /// Double-tapped in Hold mode: recording carries on without the hotkey held.
        locked: bool,
    },
    Warming,
    Processing {
        message: String,
    },
}

/// Shows the pill in `state`, or hides it when idle unless `always_show` is on.
pub fn update(app: &AppHandle, state: PillState, position: PillPosition, always_show: bool) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let _ = app.emit_to(LABEL, "pill-state", &state);
    click_state().idle_shown = matches!(state, PillState::Idle) && always_show;
    if matches!(state, PillState::Idle) && !always_show {
        let _ = window.hide();
        return;
    }
    // Only reposition when appearing, so the pill doesn't jump mid-dictation.
    if !window.is_visible().unwrap_or(false) {
        place(app, position);
        let _ = window.show();
    }
}

/// Lets the pill receive clicks and hover (while recording) or passes them
/// through to whatever is underneath (otherwise, except right over an idle
/// pill that always shows).
pub fn set_interactive(app: &AppHandle, interactive: bool) {
    let mut state = click_state();
    state.clicks = match (interactive, state.idle_shown) {
        (true, _) => Clicks::Always,
        (false, true) => Clicks::OverIdlePill,
        (false, false) => Clicks::Never,
    };
    if let Some(window) = app.get_webview_window(LABEL)
        && window.is_visible().unwrap_or(false)
    {
        let _ = window.set_ignore_cursor_events(!interactive);
        state.takes_clicks = Some(interactive);
    }
    if state.clicks == Clicks::OverIdlePill {
        drop(state);
        watch_hover(app);
    }
}

fn click_state() -> MutexGuard<'static, ClickState> {
    CLICK_STATE.lock_unpoisoned()
}

/// Starts following the cursor, once, to let an idle pill take clicks while
/// it's under the cursor.
fn watch_hover(app: &AppHandle) {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let app = app.clone();
        let spawned = thread::Builder::new()
            .name("pill-hover".into())
            .spawn(move || {
                loop {
                    thread::sleep(HOVER_POLL);
                    follow_cursor(&app);
                }
            });
        if let Err(e) = spawned {
            eprintln!("[pill] couldn't follow the cursor: {e}");
        }
    });
}

fn follow_cursor(app: &AppHandle) {
    if click_state().clicks != Clicks::OverIdlePill {
        return;
    }
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let (Ok(cursor), Ok(position), Ok(size), Ok(scale)) = (
        app.cursor_position(),
        window.outer_position(),
        window.outer_size(),
        window.scale_factor(),
    ) else {
        return;
    };
    let hovered = over_idle_pill(
        (cursor.x, cursor.y),
        (position.x, position.y),
        (size.width, size.height),
        scale,
    );
    // Checked again: dictation may have started while the cursor was read.
    let mut state = click_state();
    if state.clicks == Clicks::OverIdlePill && state.takes_clicks != Some(hovered) {
        let _ = window.set_ignore_cursor_events(!hovered);
        state.takes_clicks = Some(hovered);
    }
}

/// Whether `cursor` is over the idle pill, centered in a window at `origin`
/// of `size`, all in physical pixels.
fn over_idle_pill(cursor: (f64, f64), origin: (i32, i32), size: (u32, u32), scale: f64) -> bool {
    let center_x = f64::from(origin.0) + f64::from(size.0) / 2.0;
    let center_y = f64::from(origin.1) + f64::from(size.1) / 2.0;
    (cursor.0 - center_x).abs() <= IDLE_PILL.0 * scale / 2.0
        && (cursor.1 - center_y).abs() <= IDLE_PILL.1 * scale / 2.0
}

/// Moves the pill to `position` on the monitor under the mouse cursor.
pub fn place(app: &AppHandle, position: PillPosition) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else {
        return;
    };
    let area = monitor.work_area();
    let inset = (INSET * monitor.scale_factor()).round() as i32;
    let (x, y) = origin(
        position,
        (
            area.position.x,
            area.position.y,
            area.size.width as i32,
            area.size.height as i32,
        ),
        (size.width as i32, size.height as i32),
        inset,
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Top-left corner of the pill window within a work area, all in physical pixels
/// with y growing downwards. Same layout as the macOS app's `PillPosition`.
fn origin(
    position: PillPosition,
    (area_x, area_y, area_w, area_h): (i32, i32, i32, i32),
    (w, h): (i32, i32),
    inset: i32,
) -> (i32, i32) {
    use PillPosition::*;
    let left = area_x + inset;
    let center = area_x + (area_w - w) / 2;
    let right = area_x + area_w - w - inset;
    let top = area_y + inset;
    let middle = area_y + (area_h - h) / 2;
    let bottom = area_y + area_h - h - inset;
    match position {
        TopLeft => (left, top),
        TopCenter => (center, top),
        TopRight => (right, top),
        CenterLeft => (left, middle),
        Center => (center, middle),
        CenterRight => (right, middle),
        BottomLeft => (left, bottom),
        BottomCenter => (center, bottom),
        BottomRight => (right, bottom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_cover_all_nine_positions() {
        // A 1920x1040 work area below a 40px top bar, and a 320x56 pill.
        let area = (0, 40, 1920, 1040);
        let size = (320, 56);
        let cases = [
            (PillPosition::TopLeft, (8, 48)),
            (PillPosition::TopCenter, (800, 48)),
            (PillPosition::TopRight, (1592, 48)),
            (PillPosition::CenterLeft, (8, 532)),
            (PillPosition::Center, (800, 532)),
            (PillPosition::CenterRight, (1592, 532)),
            (PillPosition::BottomLeft, (8, 1016)),
            (PillPosition::BottomCenter, (800, 1016)),
            (PillPosition::BottomRight, (1592, 1016)),
        ];
        for (position, expected) in cases {
            assert_eq!(origin(position, area, size, 8), expected, "{position:?}");
        }
    }

    #[test]
    fn only_the_idle_pill_itself_takes_clicks() {
        // A 520x84 window at 2x, its center at (1000, 500).
        let (origin, size) = ((480, 416), (1040, 168));
        assert!(over_idle_pill((1000.0, 500.0), origin, size, 2.0));
        assert!(over_idle_pill((1060.0, 528.0), origin, size, 2.0));
        assert!(!over_idle_pill((1080.0, 500.0), origin, size, 2.0));
        assert!(!over_idle_pill((1000.0, 540.0), origin, size, 2.0));
        assert!(!over_idle_pill((500.0, 420.0), origin, size, 2.0));
    }

    #[test]
    fn origins_respect_a_secondary_monitor_offset() {
        let area = (-1280, 200, 1280, 720);
        assert_eq!(
            origin(PillPosition::TopLeft, area, (320, 56), 16),
            (-1264, 216)
        );
    }
}
