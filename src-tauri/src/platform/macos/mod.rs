mod hotkey;
mod permissions;

use enigo::{Enigo, Key};
use tauri::TitleBarStyle;

pub use hotkey::MODIFIER_HOTKEYS;
pub use permissions::{permission_settings_url, permissions, request_permission};

use super::HotkeyEvent;

/// Fn, as in the Swift app.
pub const DEFAULT_HOTKEY: &str = "Fn";

pub fn start_modifier_hotkey(
    name: &str,
    handler: Box<dyn Fn(HotkeyEvent) + Send + Sync>,
) -> Result<(), String> {
    hotkey::start(name, handler)
}

pub fn stop_modifier_hotkey() {
    hotkey::stop();
}

/// macOS virtual keycode for V (kVK_ANSI_V), the same key the Swift app sends.
const KEYCODE_V: u32 = 0x09;

pub fn send_paste_shortcut(enigo: Option<&mut Enigo>) -> Result<(), String> {
    let enigo = enigo.ok_or("Keyboard simulation is unavailable")?;
    super::press_combo(enigo, &[Key::Meta], Key::Other(KEYCODE_V))
}

/// Permissions are shown as their own screen on macOS, so there are no extra notes.
pub fn setup_notes() -> Vec<String> {
    Vec::new()
}

/// Hides the title bar and lets content run under the traffic lights, like the Swift app.
pub fn style_main_window(window: &tauri::WebviewWindow) {
    let _ = window.set_title_bar_style(TitleBarStyle::Overlay);
    let _ = window.set_title("");
}
