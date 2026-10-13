//! Which app is in front, so dictation can match its tone.

use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::NSString;

use super::focus;
use crate::platform::FrontApp;

pub fn frontmost_app() -> Option<FrontApp> {
    // Called from worker threads, which have no autorelease pool of their own.
    let (id, name) = objc2::rc::autoreleasepool(|_| {
        // SAFETY: +[NSWorkspace sharedWorkspace] returns NSWorkspace *,
        // frontmostApplication a nullable NSRunningApplication *, and
        // bundleIdentifier and localizedName nullable NSString *. Retained
        // handles the reference counts, and nil becomes None.
        unsafe {
            let workspace: Option<Retained<AnyObject>> =
                msg_send![class!(NSWorkspace), sharedWorkspace];
            let app: Option<Retained<AnyObject>> = msg_send![&*workspace?, frontmostApplication];
            let app = app?;
            let id: Option<Retained<NSString>> = msg_send![&*app, bundleIdentifier];
            let name: Option<Retained<NSString>> = msg_send![&*app, localizedName];
            Some((
                id.map(|s| s.to_string()).unwrap_or_default(),
                name.map(|s| s.to_string()).unwrap_or_default(),
            ))
        }
    })?;
    Some(FrontApp {
        id,
        name,
        // Needs Accessibility, which pasting needs anyway; empty without it.
        title: focus::focused_window_title().unwrap_or_default(),
    })
}
