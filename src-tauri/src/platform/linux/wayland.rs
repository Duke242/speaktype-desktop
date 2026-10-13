//! Pasting in Wayland sessions, which don't let apps simulate keys directly.

use std::process::{Command, Stdio};

pub fn is_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t.eq_ignore_ascii_case("wayland"))
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

pub fn has_paste_tool() -> bool {
    ["wtype", "ydotool"].iter().any(|tool| on_path(tool))
}

/// Presses Ctrl+V through a helper tool, since Wayland doesn't let apps inject
/// keys directly. Terminals can't be detected here, so they always get Ctrl+V.
pub fn send_paste() -> Result<(), String> {
    // wlroots compositors and KDE support the virtual keyboard protocol wtype uses.
    if on_path("wtype") && run("wtype", &["-M", "ctrl", "-k", "v", "-m", "ctrl"]) {
        return Ok(());
    }
    // ydotool writes to /dev/uinput through its daemon, so it works on GNOME too.
    // 29 and 47 are the Linux input codes for left Ctrl and V.
    if on_path("ydotool") && run("ydotool", &["key", "29:1", "47:1", "47:0", "29:0"]) {
        return Ok(());
    }
    Err("No Wayland paste tool worked (tried wtype and ydotool)".into())
}

/// Presses Ctrl+C, or Ctrl+Shift+C in a terminal, where Ctrl+C would stop
/// the running program.
pub fn send_copy(terminal: bool) -> Result<(), String> {
    let wtype: &[&str] = if terminal {
        &[
            "-M", "ctrl", "-M", "shift", "-k", "c", "-m", "shift", "-m", "ctrl",
        ]
    } else {
        &["-M", "ctrl", "-k", "c", "-m", "ctrl"]
    };
    if on_path("wtype") && run("wtype", wtype) {
        return Ok(());
    }
    // 29, 42 and 46 are the Linux input codes for left Ctrl, left Shift and C.
    let ydotool: &[&str] = if terminal {
        &["key", "29:1", "42:1", "46:1", "46:0", "42:0", "29:0"]
    } else {
        &["key", "29:1", "46:1", "46:0", "29:0"]
    };
    if on_path("ydotool") && run("ydotool", ydotool) {
        return Ok(());
    }
    Err("No Wayland key tool worked (tried wtype and ydotool)".into())
}

fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}
