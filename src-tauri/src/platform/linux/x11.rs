//! X11 queries. Also reaches XWayland apps in a Wayland session.

use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt},
};

use crate::platform::FrontApp;

/// Window classes of common terminal emulators, lowercase.
const TERMINALS: &[&str] = &[
    "alacritty",
    "blackbox",
    "com.mitchellh.ghostty",
    "cool-retro-term",
    "foot",
    "ghostty",
    "gnome-terminal",
    "gnome-terminal-server",
    "guake",
    "kitty",
    "konsole",
    "lxterminal",
    "mate-terminal",
    "org.gnome.console",
    "org.gnome.ptyxis",
    "org.wezfurlong.wezterm",
    "qterminal",
    "st",
    "st-256color",
    "tabby",
    "terminator",
    "terminology",
    "tilix",
    "urxvt",
    "warp",
    "wezterm",
    "xfce4-terminal",
    "xterm",
    "yakuake",
];

pub fn active_window_is_terminal() -> bool {
    active_window_classes().is_some_and(|classes| {
        classes
            .iter()
            .any(|class| TERMINALS.contains(&class.to_lowercase().as_str()))
    })
}

/// The active window's WM_CLASS instance as the id, its class as the name, and its title.
pub fn active_app() -> Option<FrontApp> {
    let window = active_window()?;
    let (conn, _) = x11rb::connect(None).ok()?;
    let classes = window_classes(&conn, window).unwrap_or_default();
    Some(FrontApp {
        id: classes.first().cloned().unwrap_or_default(),
        name: classes.get(1).cloned().unwrap_or_default(),
        title: window_title(&conn, window).unwrap_or_default(),
    })
}

/// _NET_WM_NAME, which is UTF-8, or the older WM_NAME.
fn window_title(conn: &impl Connection, window: u32) -> Option<String> {
    let utf8 = conn
        .intern_atom(false, b"UTF8_STRING")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let net_name = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let read = |property: u32, kind: u32| {
        conn.get_property(false, window, property, kind, 0, 1024)
            .ok()?
            .reply()
            .ok()
            .map(|reply| String::from_utf8_lossy(&reply.value).into_owned())
            .filter(|title| !title.is_empty())
    };
    read(net_name, utf8).or_else(|| read(AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))
}

/// Reads WM_CLASS ("instance\0class\0") of the window in _NET_ACTIVE_WINDOW.
fn active_window_classes() -> Option<Vec<String>> {
    let window = active_window()?;
    let (conn, _) = x11rb::connect(None).ok()?;
    window_classes(&conn, window)
}

fn window_classes(conn: &impl Connection, window: u32) -> Option<Vec<String>> {
    let class = conn
        .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
        .ok()?
        .reply()
        .ok()?;
    Some(
        class
            .value
            .split(|b| *b == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect(),
    )
}

pub fn active_window() -> Option<u32> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let active_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    conn.get_property(false, root, active_atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
}
