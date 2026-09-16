//! Microphone and Accessibility permission checks.

use core_foundation::{base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, string::CFString};
use objc2::{class, msg_send, runtime::{AnyObject, Bool}};

use crate::platform::{Permission, PermissionKind};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
}

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeAudio: &'static AnyObject;
}

/// AVAuthorizationStatus values.
const NOT_DETERMINED: isize = 0;
const AUTHORIZED: isize = 3;

fn microphone_status() -> isize {
    // SAFETY: AVCaptureDevice's class method takes an AVMediaType and returns an NSInteger.
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
            // SAFETY: no arguments; reads this process's trust state.
            granted: unsafe { AXIsProcessTrusted() },
        },
    ]
}

/// Shows the system prompt when macOS still allows one. Otherwise the caller
/// should open System Settings.
pub fn request_permission(kind: PermissionKind) {
    match kind {
        PermissionKind::Microphone => {
            if microphone_status() != NOT_DETERMINED {
                return;
            }
            let handler = block2::RcBlock::new(|_granted: Bool| {});
            // SAFETY: requestAccessForMediaType:completionHandler: takes an AVMediaType
            // and a block with a BOOL argument; the block is retained by AVFoundation.
            let _: () = unsafe {
                msg_send![
                    class!(AVCaptureDevice),
                    requestAccessForMediaType: AVMediaTypeAudio,
                    completionHandler: &*handler
                ]
            };
        }
        PermissionKind::Accessibility => {
            let options = CFDictionary::from_CFType_pairs(&[(
                CFString::new("AXTrustedCheckOptionPrompt").as_CFType(),
                CFBoolean::true_value().as_CFType(),
            )]);
            // SAFETY: the dictionary lives until the call returns.
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
