//! Reads the focused text field through UI Automation, so corrections made
//! after a dictation can be learned.

use windows::Win32::{
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    },
    UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
        IUIAutomationValuePattern, UIA_TextPatternId, UIA_ValuePatternId,
    },
};

/// The most text read from a document. Longer ones are read only this far.
const MAX_CHARS: i32 = 200_000;

/// Balances a successful `CoInitializeEx` on this thread.
struct Com(bool);

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: paired with the successful CoInitializeEx in `current`.
            unsafe { CoUninitialize() };
        }
    }
}

/// The text field that had keyboard focus when this was created. COM objects
/// are dropped before COM is uninitialized, as fields drop in order.
pub struct FocusedField {
    field: IUIAutomationElement,
    _com: Com,
}

impl FocusedField {
    /// The focused text field in the foreground app, if it can be read.
    /// Password fields are never read.
    pub fn current() -> Option<Self> {
        // SAFETY: initializes COM for this thread; balanced by `Com`'s drop.
        // A thread already initialized differently still gets a usable object.
        let com = Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
        // SAFETY: standard COM activation of the UI Automation client.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok()?;
        // SAFETY: plain UI Automation calls on a live client.
        let field = unsafe { automation.GetFocusedElement() }.ok()?;
        // SAFETY: as above.
        if unsafe { field.CurrentIsPassword() }.ok()?.as_bool() {
            return None;
        }
        Some(Self { field, _com: com })
    }

    /// The field's whole text, or `None` if it can't be read now. Single-line
    /// fields expose a value; documents and rich editors expose their text.
    pub fn text(&self) -> Option<String> {
        // SAFETY: plain UI Automation calls on a live element.
        unsafe {
            if let Ok(value) = self
                .field
                .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                && let Ok(text) = value.CurrentValue()
            {
                return Some(text.to_string());
            }
            let text = self
                .field
                .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                .ok()?;
            Some(
                text.DocumentRange()
                    .ok()?
                    .GetText(MAX_CHARS)
                    .ok()?
                    .to_string(),
            )
        }
    }
}
