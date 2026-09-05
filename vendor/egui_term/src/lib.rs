//! Vendored fork of [egui_term](https://github.com/Harzu/egui_term) at rev
//! `31bbc7ab8503c9518fcee5717cfa29011e59f451`, MIT licensed (see `LICENSE`).
//!
//! Patched for pomptty:
//! - M4 slice 4 ("in-terminal polish"): per-cell bold/italic font selection,
//!   a themed/shaped/blinking/gliding cursor, plain-hover OSC 8 hyperlink
//!   detection, underline (single / double / undercurl / dotted / dashed +
//!   SGR 58 underline colour) and strikeout rendering (`push_text_decoration`
//!   — `alacritty_terminal` already tracks the flags).
//! - M9: bracketed paste (`BackendCommand::Paste` / `paste_payload`);
//!   OSC 4/10/11/12 dynamic colors (`RenderableContent::colors` +
//!   `resolve_color`), plus the query form answered via `theme_rgb` +
//!   `BackendCommand::Report`; OSC 52 read direction (`BackendSettings::
//!   osc52_read`); `Ctrl+Alt`+drag block selection; `selectable_content`
//!   via `Term::selection_to_string` (multi-row copies keep line breaks);
//!   `scrollback_text` for "open scrollback in an editor"; `Event::Ime`
//!   commit handling.
//!
//! See pomptty's `ROADMAP.md` for the reasoning. Not otherwise kept in sync
//! with upstream.

mod backend;
mod bindings;
mod font;
mod theme;
mod types;
mod view;

pub use alacritty_terminal::term::ClipboardType;
pub use alacritty_terminal::vte::ansi::CursorShape;
pub use backend::settings::BackendSettings;
pub use backend::{BackendCommand, PtyEvent, TerminalBackend, TerminalMode};
pub use bindings::{Binding, BindingAction, InputKind, KeyboardBinding};
pub use font::{FontSettings, TerminalFont};
pub use theme::{ColorPalette, TerminalTheme};
pub use view::{theme_rgb, TerminalView};
