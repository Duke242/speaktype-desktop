//! Reads the focused text field through the Accessibility API, which the app
//! already needs for pasting, so corrections made after a dictation can be learned.

use std::ffi::c_void;

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    string::{CFString, CFStringRef},
};

type AXUIElementRef = *const c_void;
type AXError = i32;
const AX_SUCCESS: AXError = 0;

/// Seconds to wait for an app to answer, so a busy app can't stall the reader.
const MESSAGING_TIMEOUT: f32 = 0.5;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFRetain(value: CFTypeRef) -> CFTypeRef;
    fn CFRelease(value: CFTypeRef);
}

/// An Accessibility element we own a reference to.
struct Element(AXUIElementRef);

impl Element {
    fn attribute(&self, name: &'static str) -> Option<CFType> {
        let name = CFString::from_static_string(name);
        let mut value: CFTypeRef = std::ptr::null();
        // SAFETY: self.0 is a live AXUIElement, name a valid CFString, and on
        // success `value` holds a +1 reference that CFType takes over.
        let error = unsafe {
            AXUIElementCopyAttributeValue(self.0, name.as_concrete_TypeRef(), &mut value)
        };
        if error != AX_SUCCESS || value.is_null() {
            return None;
        }
        // SAFETY: a non-null value returned by a Copy function is owned by us.
        Some(unsafe { CFType::wrap_under_create_rule(value) })
    }

    fn string(&self, name: &'static str) -> Option<String> {
        self.attribute(name)?
            .downcast_into::<CFString>()
            .map(|s| s.to_string())
    }

    fn focused(system: &Element) -> Option<Element> {
        let focused = system.attribute("AXFocusedUIElement")?;
        // The attribute is an AXUIElement. Keep our own reference to it, as
        // dropping `focused` releases the one it holds.
        let raw = focused.as_CFTypeRef();
        // SAFETY: raw is a live CF object; CFRetain gives us a +1 reference.
        Some(Element(unsafe { CFRetain(raw) }))
    }
}

impl Drop for Element {
    fn drop(&mut self) {
        // SAFETY: we hold one reference to this element.
        unsafe { CFRelease(self.0) };
    }
}

/// The text field that had keyboard focus when this was created.
pub struct FocusedField {
    system: Element,
    field: Element,
}

impl FocusedField {
    /// The focused text field in the frontmost app, if it can be read.
    /// Password fields are never read.
    pub fn current() -> Option<Self> {
        // SAFETY: returns a +1 reference to the system-wide element.
        let system = Element(unsafe { AXUIElementCreateSystemWide() });
        if system.0.is_null() {
            return None;
        }
        // SAFETY: system.0 is a live AXUIElement.
        unsafe { AXUIElementSetMessagingTimeout(system.0, MESSAGING_TIMEOUT) };
        let field = Element::focused(&system)?;
        if field.string("AXSubrole").as_deref() == Some("AXSecureTextField")
            || field.string("AXRole").as_deref() == Some("AXSecureTextField")
        {
            return None;
        }
        // SAFETY: field.0 is a live AXUIElement.
        unsafe { AXUIElementSetMessagingTimeout(field.0, MESSAGING_TIMEOUT) };
        Some(Self { system, field })
    }

    /// The field's whole text, or `None` if it can't be read now.
    pub fn text(&self) -> Option<String> {
        self.field.string("AXValue")
    }

    /// Whether the field still has keyboard focus.
    pub fn is_focused(&self) -> bool {
        Element::focused(&self.system).is_some_and(|now| {
            // SAFETY: both are live AXUIElements.
            unsafe { CFEqual(now.0, self.field.0) != 0 }
        })
    }
}
