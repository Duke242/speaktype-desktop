//! The menu bar (macOS) or system tray (Windows, Linux) icon and its panel.
//!
//! Left-clicking the icon opens a small panel window under it (above it when the
//! taskbar is at the bottom). Right-clicking shows a short native menu. Linux
//! trays don't report clicks, so there the menu is the only option.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Rect,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    AppState,
    dictation::{Destination, DictationState, Event},
};

pub const TRAY_ID: &str = "main";
pub const PANEL_LABEL: &str = "tray";

/// Gap between the icon and the panel, in logical pixels.
const PANEL_GAP: f64 = 6.0;

/// When the panel last hid itself because it lost focus. Clicking the icon
/// while the panel is open blurs it first; without this the same click would
/// immediately reopen it.
static LAST_BLUR_HIDE_MS: AtomicU64 = AtomicU64::new(0);

// Template images: macOS tints them to match the menu bar.
const ICON_IDLE: &[u8] = include_bytes!("../icons/tray/idle.png");
const ICON_RECORDING: &[u8] = include_bytes!("../icons/tray/recording.png");
const ICON_BUSY: &[u8] = include_bytes!("../icons/tray/busy.png");

pub fn build(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open SpeakType", true, None::<&str>)?;
    let dictate = MenuItem::with_id(app, "dictate", "Start Dictation", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit SpeakType", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&dictate, &separator, &open, &settings, &separator, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("SpeakType")
        .icon(Image::from_bytes(ICON_IDLE)?)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_main_window(app, None),
            "settings" => open_main_window(app, Some("settings")),
            "dictate" => app
                .state::<AppState>()
                .controller
                .send(Event::Toggle(Destination::Paste)),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_panel(tray.app_handle(), rect);
            }
        })
        .build(app)?
        .set_visible(visible)?;
    Ok(())
}

/// Swaps the icon to show whether SpeakType is idle, recording or transcribing.
pub fn show_state(app: &AppHandle, state: &DictationState) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let bytes = match state {
        DictationState::Idle => ICON_IDLE,
        DictationState::Recording { .. } => ICON_RECORDING,
        DictationState::Transcribing { .. } => ICON_BUSY,
    };
    if let Ok(icon) = Image::from_bytes(bytes) {
        let _ = tray.set_icon_with_as_template(Some(icon), true);
    }
}

/// Shows the main window, optionally on a specific screen ("settings", "history"…).
pub fn open_main_window(app: &AppHandle, route: Option<&str>) {
    hide_panel(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        if let Some(route) = route {
            let _ = app.emit_to("main", "navigate", route);
        }
    }
}

pub fn hide_panel(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
        let _ = panel.hide();
    }
}

/// Called when the panel loses focus.
pub fn panel_blurred(app: &AppHandle) {
    LAST_BLUR_HIDE_MS.store(now_ms(), Ordering::Relaxed);
    hide_panel(app);
}

fn toggle_panel(app: &AppHandle, icon: Rect) {
    let Some(panel) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.hide();
        return;
    }
    if now_ms().saturating_sub(LAST_BLUR_HIDE_MS.load(Ordering::Relaxed)) < 300 {
        return;
    }
    if let Some(position) = panel_position(app, &panel, icon) {
        let _ = panel.set_position(position);
    }
    let _ = panel.show();
    let _ = panel.set_focus();
    let _ = app.emit_to(PANEL_LABEL, "panel-shown", ());
}

/// Centres the panel on the icon, below it when the icon is in the top half of
/// the screen (the macOS menu bar) and above it otherwise (a bottom taskbar),
/// kept inside the screen.
fn panel_position(app: &AppHandle, panel: &tauri::WebviewWindow, icon: Rect) -> Option<PhysicalPosition<i32>> {
    let scale = panel.scale_factor().ok()?;
    let icon_position = icon.position.to_physical::<f64>(scale);
    let icon_size = icon.size.to_physical::<f64>(scale);
    let size = panel.outer_size().ok()?;
    let monitor = app
        .monitor_from_point(icon_position.x, icon_position.y)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let screen = monitor.position();
    let screen_size = monitor.size();
    let gap = PANEL_GAP * scale;

    let centre_x = icon_position.x + icon_size.width / 2.0;
    let min_x = screen.x as f64 + gap;
    let max_x = (screen.x + screen_size.width as i32) as f64 - size.width as f64 - gap;
    let x = (centre_x - size.width as f64 / 2.0).clamp(min_x, max_x.max(min_x));

    let in_top_half = icon_position.y < screen.y as f64 + screen_size.height as f64 / 2.0;
    let y = if in_top_half {
        icon_position.y + icon_size.height + gap
    } else {
        icon_position.y - size.height as f64 - gap
    };
    Some(PhysicalPosition::new(x.round() as i32, y.round() as i32))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
