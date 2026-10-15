//! Reads the focused text field through the Accessibility API, which the app
//! already needs for pasting, so corrections made after a dictation can be learned.

use std::{ffi::c_void, sync::Mutex, thread, time::Duration};

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};

use crate::LockExt;

type AXUIElementRef = *const c_void;
type AXError = i32;
const AX_SUCCESS: AXError = 0;

/// Seconds to wait for an app to answer, so a busy app can't stall the reader.
const MESSAGING_TIMEOUT: f32 = 0.5;
/// How long a Chrome or Electron app gets to build its accessibility tree once asked.
const MANUAL_ACCESSIBILITY_WAIT: Duration = Duration::from_millis(100);
const MANUAL_ACCESSIBILITY_TRIES: u32 = 10;

/// Apps we switched `AXManualAccessibility` on in, by pid, with how many
/// watched fields still need it. Chrome and Electron apps slow down on every
/// keystroke while it's on, so it's switched off again once none do.
static MANUAL_ACCESSIBILITY: Mutex<Vec<(i32, usize)>> = Mutex::new(Vec::new());

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
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

    fn is_true(&self, name: &'static str) -> bool {
        self.attribute(name)
            .and_then(|value| value.downcast_into::<CFBoolean>())
            .is_some_and(bool::from)
    }

    fn set(&self, name: &'static str, on: bool) {
        let name = CFString::from_static_string(name);
        let value = if on {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        // SAFETY: self.0 is a live AXUIElement; both values are valid CF objects.
        unsafe {
            AXUIElementSetAttributeValue(self.0, name.as_concrete_TypeRef(), value.as_CFTypeRef())
        };
    }

    fn pid(&self) -> Option<i32> {
        let mut pid = 0;
        // SAFETY: self.0 is a live AXUIElement and pid a valid out pointer.
        (unsafe { AXUIElementGetPid(self.0, &mut pid) } == AX_SUCCESS).then_some(pid)
    }

    /// The focused element, if it has a text value to read.
    fn readable_focus(system: &Element) -> Option<Element> {
        Self::focused(system).filter(|field| field.string("AXValue").is_some())
    }

    fn focused(system: &Element) -> Option<Element> {
        Some(Self::retained(system.attribute("AXFocusedUIElement")?))
    }

    /// An attribute value that is itself an AXUIElement. Keeps our own
    /// reference to it, as dropping `value` releases the one it holds.
    fn retained(value: CFType) -> Element {
        let raw = value.as_CFTypeRef();
        // SAFETY: raw is a live CF object; CFRetain gives us a +1 reference.
        Element(unsafe { CFRetain(raw) })
    }
}

impl Drop for Element {
    fn drop(&mut self) {
        // SAFETY: we hold one reference to this element.
        unsafe { CFRelease(self.0) };
    }
}

/// The title of the frontmost app's focused window.
pub fn focused_window_title() -> Option<String> {
    // SAFETY: returns a +1 reference to the system-wide element.
    let system = Element(unsafe { AXUIElementCreateSystemWide() });
    if system.0.is_null() {
        return None;
    }
    // SAFETY: system.0 is a live AXUIElement.
    unsafe { AXUIElementSetMessagingTimeout(system.0, MESSAGING_TIMEOUT) };
    let app = Element::retained(system.attribute("AXFocusedApplication")?);
    let window = Element::retained(app.attribute("AXFocusedWindow")?);
    window.string("AXTitle").filter(|title| !title.is_empty())
}

/// Keeps `AXManualAccessibility` on in an app we switched it on in, and
/// switches it back off when the last one is dropped.
struct ManualAccessibility {
    app: Element,
    pid: i32,
}

impl ManualAccessibility {
    /// Another hold on an app we already switched it on in.
    fn join(app: Element) -> Option<Self> {
        let pid = app.pid()?;
        let mut held = MANUAL_ACCESSIBILITY.lock_unpoisoned();
        let (_, count) = held.iter_mut().find(|(held_pid, _)| *held_pid == pid)?;
        *count += 1;
        Some(Self { app, pid })
    }

    /// Switches it on, unless it's on already: then something else, like a
    /// screen reader, wants it, and it's left alone.
    fn switch_on(app: Element) -> Option<Self> {
        let pid = app.pid()?;
        let mut held = MANUAL_ACCESSIBILITY.lock_unpoisoned();
        if let Some((_, count)) = held.iter_mut().find(|(held_pid, _)| *held_pid == pid) {
            *count += 1;
        } else {
            if app.is_true("AXManualAccessibility") {
                return None;
            }
            app.set("AXManualAccessibility", true);
            held.push((pid, 1));
        }
        Some(Self { app, pid })
    }
}

impl Drop for ManualAccessibility {
    fn drop(&mut self) {
        let mut held = MANUAL_ACCESSIBILITY.lock_unpoisoned();
        let Some(index) = held.iter().position(|(pid, _)| *pid == self.pid) else {
            return;
        };
        held[index].1 -= 1;
        if held[index].1 == 0 {
            held.swap_remove(index);
            self.app.set("AXManualAccessibility", false);
        }
    }
}

/// The text field that had keyboard focus when this was created.
pub struct FocusedField {
    field: Element,
    /// Keeps a Chrome or Electron app's text fields readable while watched.
    _manual_accessibility: Option<ManualAccessibility>,
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
        let app = system
            .attribute("AXFocusedApplication")
            .map(Element::retained);
        let (field, manual) = match Element::readable_focus(&system) {
            Some(field) => (field, app.and_then(ManualAccessibility::join)),
            None => {
                // Chrome and Electron apps (Slack, VS Code, Discord…) only
                // expose their text fields once asked to, and build them
                // shortly after. Other apps ignore this.
                let manual = ManualAccessibility::switch_on(app?)?;
                let field = (0..MANUAL_ACCESSIBILITY_TRIES).find_map(|_| {
                    thread::sleep(MANUAL_ACCESSIBILITY_WAIT);
                    Element::readable_focus(&system)
                })?;
                (field, Some(manual))
            }
        };
        if field.string("AXSubrole").as_deref() == Some("AXSecureTextField")
            || field.string("AXRole").as_deref() == Some("AXSecureTextField")
        {
            return None;
        }
        // SAFETY: field.0 is a live AXUIElement.
        unsafe { AXUIElementSetMessagingTimeout(field.0, MESSAGING_TIMEOUT) };
        Some(Self {
            field,
            _manual_accessibility: manual,
        })
    }

    /// The field's whole text, or `None` if it can't be read now.
    pub fn text(&self) -> Option<String> {
        self.field.string("AXValue")
    }
}
