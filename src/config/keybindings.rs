//! Configurable keyboard shortcuts.
//!
//! The config maps a chord string like `"ctrl+shift+t"` to an [`Action`]. These
//! are *application* actions (new tab, font size, history search, …); ordinary
//! keystrokes and terminal-level shortcuts such as copy/paste are handled by the
//! terminal widget itself.

use std::collections::BTreeMap;

use egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};

/// Something the app can do in response to a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Copy,
    Paste,
    FontIncrease,
    FontDecrease,
    FontReset,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    Clear,
    ReloadConfig,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    /// Jump to tab N (1-based); a value past the last tab jumps to the last.
    /// In JSON: `{ "goto-tab": 3 }`.
    GotoTab(u8),
    /// Open the `Ctrl+R` history search — reserved; wired up in the history
    /// milestone. Until then the binding is inert and the keystroke is passed
    /// through to the shell.
    HistorySearch,
}

impl Action {
    /// Whether this action does something in the current milestone. Inert
    /// actions are still parsed and stored, but do not consume the keystroke.
    pub fn is_active(self) -> bool {
        !matches!(
            self,
            // Copy/Paste are handled inside the terminal widget itself, so the
            // app layer leaves those keystrokes alone.
            Action::Copy | Action::Paste | Action::HistorySearch
        )
    }
}

/// A parsed key chord: an exact modifier set plus a logical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub modifiers: Modifiers,
    pub key: Key,
}

/// Parse a chord string such as `"ctrl+shift+t"`, `"ctrl+plus"`, `"shift+pageup"`.
///
/// Recognized modifiers: `ctrl`/`control`, `shift`, `alt`/`option`,
/// `super`/`cmd`/`meta`/`win`. The final token is the key.
pub fn parse_chord(s: &str) -> Result<Chord, String> {
    let mut modifiers = Modifiers::default();
    let mut key: Option<Key> = None;

    for raw in s.split('+') {
        let tok = raw.trim().to_ascii_lowercase();
        if tok.is_empty() {
            return Err(format!("empty token in chord {s:?}"));
        }
        match tok.as_str() {
            "ctrl" | "control" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" | "option" => modifiers.alt = true,
            "super" | "cmd" | "command" | "meta" | "win" | "windows" => {
                modifiers.mac_cmd = true;
                modifiers.command = true;
            }
            other => {
                if key.is_some() {
                    return Err(format!("chord {s:?} has more than one key"));
                }
                key = Some(
                    parse_key(other)
                        .ok_or_else(|| format!("unknown key {other:?} in chord {s:?}"))?,
                );
            }
        }
    }

    match key {
        Some(key) => Ok(Chord { modifiers, key }),
        None => Err(format!("chord {s:?} has no key")),
    }
}

fn parse_key(tok: &str) -> Option<Key> {
    Some(match tok {
        "enter" | "return" => Key::Enter,
        "space" | "spacebar" => Key::Space,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "escape" | "esc" => Key::Escape,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "home" => Key::Home,
        "end" => Key::End,
        "insert" | "ins" => Key::Insert,
        "delete" | "del" => Key::Delete,
        "up" | "arrowup" => Key::ArrowUp,
        "down" | "arrowdown" => Key::ArrowDown,
        "left" | "arrowleft" => Key::ArrowLeft,
        "right" | "arrowright" => Key::ArrowRight,
        "plus" | "add" => Key::Plus,
        "minus" | "subtract" | "-" => Key::Minus,
        "equals" | "equal" | "=" => Key::Equals,
        "," | "comma" => Key::Comma,
        "." | "period" => Key::Period,
        "/" | "slash" => Key::Slash,
        "\\" | "backslash" => Key::Backslash,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphabetic() => {
            Key::from_name(&s.to_ascii_uppercase())?
        }
        s if s.starts_with('f') && s[1..].parse::<u8>().is_ok() => {
            Key::from_name(&format!("F{}", &s[1..]))?
        }
        _ => return None,
    })
}

/// The keybinding table: chord string -> action.
///
/// Stored as a `BTreeMap` keyed by the *original* chord string so it round-trips
/// through JSON exactly as the user wrote it; parsing happens on lookup/compile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyBindings(pub BTreeMap<String, Action>);

impl KeyBindings {
    #[allow(dead_code)] // used in tests and by the config editor later
    pub fn get(&self, chord: &str) -> Option<Action> {
        self.0.get(chord).copied()
    }

    /// Parse every binding, returning `(Chord, Action)` pairs and logging (not
    /// failing on) any malformed chord strings.
    pub fn compile(&self) -> Vec<(Chord, Action)> {
        let mut out = Vec::with_capacity(self.0.len());
        for (chord, action) in &self.0 {
            match parse_chord(chord) {
                Ok(c) => out.push((c, *action)),
                Err(e) => log::warn!("ignoring keybinding: {e}"),
            }
        }
        out
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        let defaults: &[(&str, Action)] = &[
            ("ctrl+shift+c", Action::Copy),
            ("ctrl+shift+v", Action::Paste),
            ("ctrl+plus", Action::FontIncrease),
            ("ctrl+equals", Action::FontIncrease),
            ("ctrl+minus", Action::FontDecrease),
            ("ctrl+0", Action::FontReset),
            ("shift+pageup", Action::ScrollPageUp),
            ("shift+pagedown", Action::ScrollPageDown),
            ("shift+home", Action::ScrollToTop),
            ("shift+end", Action::ScrollToBottom),
            ("ctrl+shift+k", Action::Clear),
            ("ctrl+shift+r", Action::ReloadConfig),
            ("ctrl+shift+t", Action::NewTab),
            ("ctrl+shift+w", Action::CloseTab),
            ("ctrl+shift+pagedown", Action::NextTab),
            ("ctrl+shift+pageup", Action::PrevTab),
            ("ctrl+tab", Action::NextTab),
            ("ctrl+shift+tab", Action::PrevTab),
            ("ctrl+1", Action::GotoTab(1)),
            ("ctrl+2", Action::GotoTab(2)),
            ("ctrl+3", Action::GotoTab(3)),
            ("ctrl+4", Action::GotoTab(4)),
            ("ctrl+5", Action::GotoTab(5)),
            ("ctrl+6", Action::GotoTab(6)),
            ("ctrl+7", Action::GotoTab(7)),
            ("ctrl+8", Action::GotoTab(8)),
            ("ctrl+9", Action::GotoTab(9)),
            ("ctrl+r", Action::HistorySearch),
        ];
        KeyBindings(
            defaults
                .iter()
                .map(|(k, v)| ((*k).to_owned(), *v))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_chords() {
        let c = parse_chord("ctrl+shift+t").unwrap();
        assert!(c.modifiers.ctrl && c.modifiers.shift && !c.modifiers.alt);
        assert_eq!(c.key, Key::T);
    }

    #[test]
    fn parses_symbolic_keys() {
        assert_eq!(parse_chord("ctrl+plus").unwrap().key, Key::Plus);
        assert_eq!(parse_chord("ctrl+0").unwrap().key, Key::Num0);
        assert_eq!(parse_chord("shift+pageup").unwrap().key, Key::PageUp);
        assert_eq!(parse_chord("f5").unwrap().key, Key::F5);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_chord("ctrl+").is_err());
        assert!(parse_chord("ctrl+nope").is_err());
        assert!(parse_chord("shift").is_err());
        assert!(parse_chord("a+b").is_err());
    }

    #[test]
    fn default_bindings_all_compile() {
        let kb = KeyBindings::default();
        assert_eq!(kb.compile().len(), kb.0.len());
    }

    #[test]
    fn bindings_roundtrip_json() {
        let kb = KeyBindings::default();
        let text = serde_json::to_string(&kb).unwrap();
        let back: KeyBindings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.0, kb.0);
    }

    #[test]
    fn goto_tab_serializes_as_object() {
        let json = serde_json::to_string(&Action::GotoTab(3)).unwrap();
        assert_eq!(json, r#"{"goto-tab":3}"#);
        let back: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Action::GotoTab(3));
    }

    #[test]
    fn tab_actions_are_active() {
        assert!(Action::NewTab.is_active());
        assert!(Action::NextTab.is_active());
        assert!(Action::GotoTab(2).is_active());
        assert!(!Action::HistorySearch.is_active());
    }
}
