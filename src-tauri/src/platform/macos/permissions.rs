//! Microphone and Accessibility permission checks.

use core_foundation::{
    base::{Boolean, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use objc2::{class, msg_send, runtime::Bool};
use objc2_foundation::NSString;

use crate::platform::{Permission, PermissionKind};

// Signatures from HIServices' AXUIElement.h, which return Boolean (an unsigned
// char), not C99 bool.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> Boolean;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeAudio: &'static NSString;
}

/// AVAuthorizationStatus values.
const NOT_DETERMINED: isize = 0;
const AUTHORIZED: isize = 3;

fn microphone_status() -> isize {
    // SAFETY: +[AVCaptureDevice authorizationStatusForMediaType:] takes an
    // AVMediaType and returns an AVAuthorizationStatus (NSInteger).
    unsafe { msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: AVMediaTypeAudio] }
}

pub fn permissions() -> Vec<Permission> {
    vec![
        Permission {
            kind: PermissionKind::Microphone,
            granted: microphone_status() == AUTHORIZED,
        },
        Permission {
            kind: PermissionKind::Accessibility,
            // SAFETY: takes no arguments and reads this process's trust state.
            granted: unsafe { AXIsProcessTrusted() } != 0,
        },
    ]
}

/// Shows the system prompt when macOS still allows one. Otherwise the caller
/// should open System Settings. Must run on the main thread.
pub fn request_permission(kind: PermissionKind) {
    match kind {
        PermissionKind::Microphone => {
            if microphone_status() != NOT_DETERMINED {
                return;
            }
            let handler = block2::RcBlock::new(|_granted: Bool| {});
            // SAFETY: +[AVCaptureDevice requestAccessForMediaType:completionHandler:]
            // takes an AVMediaType and a `void (^)(BOOL)` block, which it copies.
            let _: () = unsafe {
                msg_send![
                    class!(AVCaptureDevice),
                    requestAccessForMediaType: AVMediaTypeAudio,
                    completionHandler: &*handler
                ]
            };
        }
        PermissionKind::Accessibility => {
            // SAFETY: kAXTrustedCheckOptionPrompt is a CFString constant that
            // lives for the whole process.
            let prompt = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
            let options = CFDictionary::from_CFType_pairs(&[(prompt, CFBoolean::true_value())]);
            // SAFETY: the dictionary is valid and outlives the call.
            unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
        }
    }
}

pub fn permission_settings_url(kind: PermissionKind) -> Option<&'static str> {
    Some(match kind {
        PermissionKind::Microphone => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        PermissionKind::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
    })
}
