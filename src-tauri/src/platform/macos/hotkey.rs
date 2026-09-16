//! Hotkeys made of a single modifier key (Fn, Right ⌘…), which the regular
//! global shortcut API can't express. A session event tap watches modifier
//! changes on its own thread, ported from the Swift app's AppDelegate.

use std::{
    ffi::c_void,
    ptr,
    sync::{
        Mutex, Once,
        atomic::{AtomicPtr, Ordering},
    },
    thread,
    time::Duration,
};

use objc2::{class, msg_send, runtime::AnyObject};
use objc2_foundation::NSString;

use crate::platform::HotkeyEvent;

pub const MODIFIER_HOTKEYS: &[&str] = &[
    "Fn",
    "RightCommand",
    "LeftCommand",
    "RightOption",
    "LeftOption",
    "RightControl",
    "LeftControl",
];

type Handler = Box<dyn Fn(HotkeyEvent) + Send + Sync>;

struct Active {
    key: ModifierKey,
    handler: Handler,
    pressed: bool,
}

static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);
static TAP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static START_TAP: Once = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierKey {
    Fn,
    RightCommand,
    LeftCommand,
    RightOption,
    LeftOption,
    RightControl,
    LeftControl,
}

impl ModifierKey {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "Fn" => Self::Fn,
            "RightCommand" => Self::RightCommand,
            "LeftCommand" => Self::LeftCommand,
            "RightOption" => Self::RightOption,
            "LeftOption" => Self::LeftOption,
            "RightControl" => Self::RightControl,
            "LeftControl" => Self::LeftControl,
            _ => return None,
        })
    }

    /// macOS virtual keycodes.
    fn keycode(self) -> i64 {
        match self {
            Self::Fn => 63,
            Self::RightCommand => 54,
            Self::LeftCommand => 55,
            Self::RightOption => 61,
            Self::LeftOption => 58,
            Self::RightControl => 62,
            Self::LeftControl => 59,
        }
    }

    /// Device-specific flag bits, so Right ⌘ isn't confused with Left ⌘.
    fn is_down(self, flags: u64) -> bool {
        let mask = match self {
            Self::Fn => 0x0080_0000, // kCGEventFlagMaskSecondaryFn
            Self::RightCommand => 0x10,
            Self::LeftCommand => 0x08,
            Self::RightOption => 0x40,
            Self::LeftOption => 0x20,
            Self::RightControl => 0x2000,
            Self::LeftControl => 0x01,
        };
        flags & mask != 0
    }
}

pub fn start(name: &str, handler: Handler) -> Result<(), String> {
    let key = ModifierKey::parse(name).ok_or_else(|| format!("{name} isn't a modifier key"))?;
    *ACTIVE.lock().unwrap() = Some(Active { key, handler, pressed: false });
    START_TAP.call_once(|| {
        thread::Builder::new()
            .name("modifier-hotkey".into())
            .spawn(run_tap)
            .expect("failed to start hotkey thread");
    });
    Ok(())
}

pub fn stop() {
    *ACTIVE.lock().unwrap() = None;
}

type CFTypeRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, key_down: bool) -> *mut c_void;
    fn CGEventPost(tap: u32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFMachPortCreateRunLoopSource(allocator: *mut c_void, port: *mut c_void, order: isize) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: CFTypeRef);
    fn CFRunLoopRun();
    fn CFRelease(value: CFTypeRef);
    static kCFRunLoopCommonModes: CFTypeRef;
}

const SESSION_EVENT_TAP: u32 = 1;
const HID_EVENT_TAP: u32 = 0;
const HEAD_INSERT: u32 = 0;
const KEY_DOWN: u32 = 10;
const FLAGS_CHANGED: u32 = 12;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
const KEYCODE_FIELD: u32 = 9;
/// F19, posted to break the Globe key's "show emoji picker" gesture.
const KEYCODE_F19: u16 = 0x50;

/// Creates the tap and runs its run loop forever. Without Accessibility
/// permission creation fails, so it retries until the user grants it.
fn run_tap() {
    loop {
        // SAFETY: plain CoreGraphics/CoreFoundation calls; the callback has the
        // CGEventTapCallBack signature and the port lives for the process.
        unsafe {
            let mask = (1u64 << FLAGS_CHANGED) | (1u64 << KEY_DOWN);
            let port = CGEventTapCreate(SESSION_EVENT_TAP, HEAD_INSERT, 0, mask, on_event, ptr::null_mut());
            if !port.is_null() {
                TAP.store(port, Ordering::Relaxed);
                let source = CFMachPortCreateRunLoopSource(ptr::null_mut(), port, 0);
                CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
                CGEventTapEnable(port, true);
                CFRunLoopRun();
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
}

unsafe extern "C" fn on_event(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    _user_info: *mut c_void,
) -> *mut c_void {
    // macOS switches slow taps off; switch it straight back on.
    if event_type == TAP_DISABLED_BY_TIMEOUT || event_type == TAP_DISABLED_BY_USER_INPUT {
        let port = TAP.load(Ordering::Relaxed);
        if !port.is_null() {
            unsafe { CGEventTapEnable(port, true) };
        }
        return event;
    }

    let Ok(mut guard) = ACTIVE.lock() else {
        return event;
    };
    let Some(active) = guard.as_mut() else {
        return event;
    };
    let keycode = unsafe { CGEventGetIntegerValueField(event, KEYCODE_FIELD) };

    match event_type {
        FLAGS_CHANGED if keycode == active.key.keycode() => {
            let down = active.key.is_down(unsafe { CGEventGetFlags(event) });
            if down != active.pressed {
                active.pressed = down;
                (active.handler)(if down { HotkeyEvent::Down } else { HotkeyEvent::Up });
                if down && active.key == ModifierKey::Fn {
                    thread::spawn(suppress_emoji_picker);
                }
            }
            // Swallow Fn so terminals don't receive stray escape sequences.
            if active.key == ModifierKey::Fn {
                return ptr::null_mut();
            }
            event
        }
        // Another key while the hotkey is held means it's part of a normal
        // shortcut (⌘C with ⌘ as the hotkey), not a dictation.
        KEY_DOWN if active.pressed && keycode != KEYCODE_F19 as i64 => {
            active.pressed = false;
            (active.handler)(HotkeyEvent::Interrupted);
            event
        }
        _ => event,
    }
}

const TERMINALS: &[&str] = &[
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "com.mitchellh.ghostty",
    "io.alacritty",
    "org.alacritty",
    "net.kovidgoyal.kitty",
    "com.github.wez.wezterm",
    "dev.warp.Warp-Stable",
    "dev.warp.Warp",
    "co.zeit.hyper",
    "org.tabby",
    "com.microsoft.VSCode",
    "com.vscodium",
    "com.todesktop.230313mzl4w4u92",
];

/// Taps F19 so holding Fn doesn't open the emoji picker, unless a terminal is
/// in front (it would print the key) or the Globe key isn't set to show emoji.
fn suppress_emoji_picker() {
    if frontmost_bundle_id().is_some_and(|id| TERMINALS.contains(&id.as_str())) {
        return;
    }
    if globe_key_usage().is_some_and(|usage| usage != 2) {
        return;
    }
    // SAFETY: creates, posts and releases two keyboard events.
    unsafe {
        for down in [true, false] {
            let event = CGEventCreateKeyboardEvent(ptr::null_mut(), KEYCODE_F19, down);
            if !event.is_null() {
                CGEventPost(HID_EVENT_TAP, event);
                CFRelease(event);
            }
        }
    }
}

fn frontmost_bundle_id() -> Option<String> {
    // SAFETY: NSWorkspace calls that return autoreleased objects or nil.
    unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let id: *const NSString = msg_send![app, bundleIdentifier];
        id.as_ref().map(|id| id.to_string())
    }
}

/// `AppleFnUsageType` from the HIToolbox preferences: 2 means "Show Emoji & Symbols".
fn globe_key_usage() -> Option<i64> {
    use core_foundation::{
        base::{CFType, TCFType},
        number::CFNumber,
        string::CFString,
    };
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFPreferencesCopyAppValue(key: CFTypeRef, app: CFTypeRef) -> CFTypeRef;
    }
    let key = CFString::new("AppleFnUsageType");
    let domain = CFString::new("com.apple.HIToolbox");
    // SAFETY: returns a +1 reference or null, which the wrapper takes ownership of.
    let value = unsafe { CFPreferencesCopyAppValue(key.as_CFTypeRef(), domain.as_CFTypeRef()) };
    if value.is_null() {
        return None;
    }
    let value = unsafe { CFType::wrap_under_create_rule(value) };
    value.downcast::<CFNumber>().and_then(|n| n.to_i64())
}
