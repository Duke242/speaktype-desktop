//! Reading other apps' text fields would need AT-SPI, which many Linux apps
//! don't support reliably, so corrections are only learned from History here.

/// Never created on Linux.
pub struct FocusedField(());

impl FocusedField {
    pub fn current(_chromium_apps: bool) -> Option<Self> {
        None
    }

    pub fn text(&self) -> Option<String> {
        None
    }
}
