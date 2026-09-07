//! Vendored fork of [egui_term](https://github.com/Harzu/egui_term) at rev
//! `31bbc7ab8503c9518fcee5717cfa29011e59f451`, MIT licensed (see `LICENSE`).
//!
//! Patched for pomptty:
//! - M4 slice 4 ("in-terminal polish"): per-cell bold/italic font selection,
//!   a themed/shaped/gliding cursor that blinks as a pixel-snapped fade (a
//!   torn window blit under a no-vsync compositor then shows two near-equal
//!   opacities, not a striped half-block), plain-hover OSC 8 hyperlink
//!   detection, underline (single / double / undercurl / dotted / dashed +
//!   SGR 58 underline colour) and strikeout rendering (`push_text_decoration`
//!   — `alacritty_terminal` already tracks the flags), pomptty-drawn
//!   box-drawing / block / shade / Powerline glyphs (`box_drawing`), a
//!   `bold_is_bright` option, and `set_bg_opacity` (skip the opaque grid
//!   background for a translucent window / wallpaper).
//! - M9: bracketed paste (`BackendCommand::Paste` / `paste_payload`, which
//!   also drops a trailing newline so a whole-line paste never presses Return);
//!   OSC 4/10/11/12 dynamic colors (`RenderableContent::colors` +
//!   `resolve_color`), plus the query form answered via `theme_rgb` +
//!   `BackendCommand::Report`; OSC 52 read direction (`BackendSettings::
//!   osc52_read`); `Ctrl+Alt`+drag block selection; `selectable_content`
//!   via `Term::selection_to_string` (multi-row copies keep line breaks);
//!   `scrollback_text` for "open scrollback in an editor"; `Event::Ime`
//!   commit handling.
//! - Keyboard input is gated on grid focus alone, not on the pointer also
//!   being over the grid (`process_input`) — so you can type as soon as the
//!   window is focused. Mouse events still require the pointer / an active
//!   drag.
//!
//! See pomptty's `ROADMAP.md` for the reasoning. Not otherwise kept in sync
//! with upstream.

mod backend;
mod bindings;
mod box_drawing;
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
