//! Vendored fork of [egui_term](https://github.com/Harzu/egui_term) at rev
//! `31bbc7ab8503c9518fcee5717cfa29011e59f451`, MIT licensed (see `LICENSE`).
//!
//! Patched for pomptty:
//! - M4 slice 4 ("in-terminal polish"): per-cell bold/italic font selection,
//!   a themed/shaped/blinking/gliding cursor, plain-hover OSC 8 hyperlink
//!   detection.
//! - M9: bracketed paste (`BackendCommand::Paste` / `paste_payload`);
//!   OSC 4/10/11/12 dynamic colors (`RenderableContent::colors` +
//!   `resolve_color`); `Ctrl+Alt`+drag block selection; `selectable_content`
//!   via `Term::selection_to_string` (multi-row copies keep line breaks).
//!
//! See pomptty's `ROADMAP.md` for the reasoning. Not otherwise kept in sync
//! with upstream.

mod backend;
mod bindings;
mod font;
mod theme;
mod types;
mod view;

pub use alacritty_terminal::vte::ansi::CursorShape;
pub use backend::settings::BackendSettings;
pub use backend::{BackendCommand, PtyEvent, TerminalBackend, TerminalMode};
pub use bindings::{Binding, BindingAction, InputKind, KeyboardBinding};
pub use font::{FontSettings, TerminalFont};
pub use theme::{ColorPalette, TerminalTheme};
pub use view::TerminalView;
