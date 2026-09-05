//! X11/Wayland primary selection: the "select to copy, middle-click to paste"
//! convention. Separate from the `CLIPBOARD` egui already handles.
//!
//! `egui`'s clipboard API only touches `CLIPBOARD`, so this uses `arboard`
//! directly. The [`PrimarySelection`] handle is held for the process
//! lifetime — on X11 `arboard` keeps a background thread that owns the
//! selection and serves paste requests as long as the handle is alive.
//!
//! On platforms without a primary selection (Windows, macOS) every method is
//! a no-op.

#[cfg(target_os = "linux")]
pub use imp::PrimarySelection;

#[cfg(not(target_os = "linux"))]
pub use stub::PrimarySelection;

#[cfg(target_os = "linux")]
mod imp {
    use arboard::{Clipboard, LinuxClipboardKind, SetExtLinux};

    pub struct PrimarySelection {
        clipboard: Option<Clipboard>,
        /// The text we last wrote, so a redundant per-frame re-set is skipped.
        last_set: String,
    }

    impl PrimarySelection {
        pub fn new() -> Self {
            let clipboard = Clipboard::new()
                .inspect_err(|e| log::warn!("primary selection unavailable: {e}"))
                .ok();
            Self {
                clipboard,
                last_set: String::new(),
            }
        }

        /// Mirror the current mouse selection onto `PRIMARY`. Cheap to call
        /// every frame — a no-op unless `text` actually changed.
        pub fn set(&mut self, text: &str) {
            if text.is_empty() || text == self.last_set {
                return;
            }
            let Some(clipboard) = &mut self.clipboard else {
                return;
            };
            match clipboard
                .set()
                .clipboard(LinuxClipboardKind::Primary)
                .text(text.to_owned())
            {
                Ok(()) => self.last_set = text.to_owned(),
                Err(e) => log::debug!("could not set primary selection: {e}"),
            }
        }

        /// The current `PRIMARY` contents, for a middle-click paste.
        pub fn get(&mut self) -> Option<String> {
            use arboard::GetExtLinux;
            self.clipboard
                .as_mut()?
                .get()
                .clipboard(LinuxClipboardKind::Primary)
                .text()
                .ok()
                .filter(|s| !s.is_empty())
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod stub {
    pub struct PrimarySelection;

    impl PrimarySelection {
        pub fn new() -> Self {
            Self
        }
        pub fn set(&mut self, _text: &str) {}
        pub fn get(&mut self) -> Option<String> {
            None
        }
    }
}
