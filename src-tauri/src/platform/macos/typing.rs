//! Unicode text events avoid the clipboard and ignore the held Fn modifier.
use enigo::{Enigo, Keyboard, Settings};
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};

pub const LIVE_EVENT_TAG: i64 = 0x5350_4b54;
pub const SOURCE_USER_DATA: u32 = 42;

pub fn typing_target() -> Option<u64> {
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: NSWorkspace exposes these selectors; nil is handled by Option.
        unsafe {
            let workspace: Option<Retained<AnyObject>> =
                msg_send![class!(NSWorkspace), sharedWorkspace];
            let app: Option<Retained<AnyObject>> = msg_send![&*workspace?, frontmostApplication];
            let pid: i32 = msg_send![&*app?, processIdentifier];
            (pid > 0).then_some(pid as u64)
        }
    })
}

pub fn send_live_text(enigo: &mut Option<Enigo>, text: &str) -> Result<(), String> {
    if enigo.is_none() {
        *enigo = Some(
            Enigo::new(&Settings {
                event_source_user_data: Some(LIVE_EVENT_TAG),
                ..Settings::default()
            })
            .map_err(|e| e.to_string())?,
        );
    }
    // Enigo's Unicode path clears modifier flags and preserves Unicode scalars.
    enigo
        .as_mut()
        .ok_or("Keyboard simulation unavailable")?
        .text(text)
        .map_err(|e| e.to_string())
}
