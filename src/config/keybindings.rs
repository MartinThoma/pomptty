//! Configurable keyboard shortcuts.
//!
//! The config maps a chord string like `"ctrl+shift+t"` to an [`Action`]. These
//! are *application* actions (new tab, font size, history search, …); ordinary
//! keystrokes and terminal-level shortcuts such as copy/paste are handled by the
//! terminal widget itself.

use std::borrow::Cow;
use std::collections::BTreeMap;

use egui::{Key, Modifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Something the app can do in response to a shortcut.
///
/// Serialized as a kebab-case string (`"new-tab"`, `"font-increase"`, …);
/// [`Action::GotoTab`] round-trips as `"goto-tab-1"` … `"goto-tab-9"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Reopen the most recently closed tab (in its old slot, same directory),
    /// or open a new tab when nothing has been closed.
    ReopenTab,
    CloseTab,
    NextTab,
    PrevTab,
    /// Open the fuzzy tab switcher.
    TabSearch,
    /// Jump to tab N (1-based); a value past the last tab jumps to the last.
    /// In the config: `"goto-tab-3"`.
    GotoTab(u8),
    /// Open the `Ctrl+R` fuzzy history-search overlay. With
    /// `"history": { "enabled": false }` in the config the keystroke is handed
    /// to the shell's own reverse-i-search instead.
    HistorySearch,
    /// Open the command palette: one input over actions, tabs, history and
    /// recent directories.
    Omnibox,
    /// Maximize the window.
    WindowMaximize,
    /// Restore the window to its pre-maximize size.
    WindowRestore,
    /// Un-maximize and tile the window to the left half of the screen.
    WindowLeftHalf,
    /// Un-maximize and tile the window to the right half of the screen.
    WindowRightHalf,
    /// Dump the scrollback of the active tab to a temp file and open it with
    /// the system's default handler.
    OpenScrollback,
    /// Turn a default binding off. Put `"<chord>": "disabled"` in the config to
    /// suppress a shortcut that would otherwise come from the defaults.
    Disabled,
}

impl Action {
    fn as_str(self) -> Cow<'static, str> {
        match self {
            Action::Copy => "copy".into(),
            Action::Paste => "paste".into(),
            Action::FontIncrease => "font-increase".into(),
            Action::FontDecrease => "font-decrease".into(),
            Action::FontReset => "font-reset".into(),
            Action::ScrollPageUp => "scroll-page-up".into(),
            Action::ScrollPageDown => "scroll-page-down".into(),
            Action::ScrollToTop => "scroll-to-top".into(),
            Action::ScrollToBottom => "scroll-to-bottom".into(),
            Action::Clear => "clear".into(),
            Action::ReloadConfig => "reload-config".into(),
            Action::NewTab => "new-tab".into(),
            Action::ReopenTab => "reopen-tab".into(),
            Action::CloseTab => "close-tab".into(),
            Action::NextTab => "next-tab".into(),
            Action::PrevTab => "prev-tab".into(),
            Action::TabSearch => "tab-search".into(),
            Action::GotoTab(n) => format!("goto-tab-{n}").into(),
            Action::HistorySearch => "history-search".into(),
            Action::Omnibox => "omnibox".into(),
            Action::WindowMaximize => "window-maximize".into(),
            Action::WindowRestore => "window-restore".into(),
            Action::WindowLeftHalf => "window-left-half".into(),
            Action::WindowRightHalf => "window-right-half".into(),
            Action::OpenScrollback => "open-scrollback".into(),
            Action::Disabled => "disabled".into(),
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "copy" => Action::Copy,
            "paste" => Action::Paste,
            "font-increase" => Action::FontIncrease,
            "font-decrease" => Action::FontDecrease,
            "font-reset" => Action::FontReset,
            "scroll-page-up" => Action::ScrollPageUp,
            "scroll-page-down" => Action::ScrollPageDown,
            "scroll-to-top" => Action::ScrollToTop,
            "scroll-to-bottom" => Action::ScrollToBottom,
            "clear" => Action::Clear,
            "reload-config" => Action::ReloadConfig,
            "new-tab" => Action::NewTab,
            "reopen-tab" => Action::ReopenTab,
            "close-tab" => Action::CloseTab,
            "next-tab" => Action::NextTab,
            "prev-tab" => Action::PrevTab,
            "tab-search" => Action::TabSearch,
            "history-search" => Action::HistorySearch,
            "omnibox" => Action::Omnibox,
            "window-maximize" => Action::WindowMaximize,
            "window-restore" => Action::WindowRestore,
            "window-left-half" => Action::WindowLeftHalf,
            "window-right-half" => Action::WindowRightHalf,
            "open-scrollback" => Action::OpenScrollback,
            "disabled" => Action::Disabled,
            other => Action::GotoTab(other.strip_prefix("goto-tab-")?.parse().ok()?),
        })
    }

    /// Whether this action does something in the current milestone. Inert
    /// actions are still parsed and stored, but do not consume the keystroke.
    pub fn is_active(self) -> bool {
        !matches!(
            self,
            // Copy/Paste are handled inside the terminal widget itself, so the
            // app layer leaves those keystrokes alone.
            Action::Copy | Action::Paste | Action::Disabled
        )
    }
}

impl Serialize for Action {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Action::parse(&s).ok_or_else(|| D::Error::custom(format!("unknown action {s:?}")))
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

/// Render a config chord string (`"ctrl+shift+t"`) for display
/// (`"Ctrl+Shift+T"`) — used in the command palette.
pub fn pretty_chord(chord: &str) -> String {
    chord
        .split('+')
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_owned(),
            "shift" => "Shift".to_owned(),
            "alt" | "option" => "Alt".to_owned(),
            "super" | "cmd" | "command" | "meta" | "win" | "windows" => "Super".to_owned(),
            "pageup" | "pgup" => "PgUp".to_owned(),
            "pagedown" | "pgdn" => "PgDn".to_owned(),
            "plus" | "add" => "+".to_owned(),
            "minus" | "subtract" => "-".to_owned(),
            "equals" | "equal" => "=".to_owned(),
            "escape" | "esc" => "Esc".to_owned(),
            other if other.chars().count() == 1 => other.to_uppercase(),
            other => {
                let mut it = other.chars();
                match it.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + it.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
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

    /// The effective binding table: the built-in defaults, with the config's
    /// entries layered on top. A config entry overrides or adds a chord;
    /// `Action::Disabled` removes a default. This means new default shortcuts in
    /// a future version show up automatically without regenerating `config.json`.
    pub fn merged(&self) -> BTreeMap<String, Action> {
        let mut merged = Self::default().0;
        for (chord, action) in &self.0 {
            if *action == Action::Disabled {
                merged.remove(chord);
            } else {
                merged.insert(chord.clone(), *action);
            }
        }
        merged
    }

    /// Parse the [merged](Self::merged) bindings into `(Chord, Action)` pairs,
    /// logging (not failing on) any malformed chord strings.
    pub fn compile(&self) -> Vec<(Chord, Action)> {
        let merged = self.merged();
        let mut out = Vec::with_capacity(merged.len());
        for (chord, action) in &merged {
            match parse_chord(chord) {
                Ok(c) => out.push((c, *action)),
                Err(e) => log::warn!("ignoring keybinding {chord:?}: {e}"),
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
            ("ctrl+shift+t", Action::ReopenTab),
            ("ctrl+shift+a", Action::TabSearch),
            ("ctrl+w", Action::CloseTab),
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
            ("ctrl+shift+p", Action::Omnibox),
        ];
        KeyBindings(
            defaults
                .iter()
                .map(|(k, v)| ((*k).to_owned(), *v))
                .collect(),
        )
    }
}

/// A table of chords that send raw bytes / an escape sequence to the PTY,
/// rather than triggering an app [`Action`]. Config key `key_sends`:
///
/// ```json
/// "key_sends": { "alt+left": "b", "ctrl+alt+k": "\r\n" }
/// ```
///
/// The value string supports `\e` / `\x1b`, `\xNN`, `\u{NNNN}`, `\n`, `\r`,
/// `\t`, `\0` and `\\`; any other text is sent as-is (UTF-8).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeySends(pub BTreeMap<String, String>);

impl KeySends {
    /// Parse into `(Chord, bytes)` pairs, logging (not failing on) a
    /// malformed chord.
    pub fn compile(&self) -> Vec<(Chord, Vec<u8>)> {
        let mut out = Vec::with_capacity(self.0.len());
        for (chord, seq) in &self.0 {
            match parse_chord(chord) {
                Ok(c) => out.push((c, parse_escapes(seq))),
                Err(e) => log::warn!("ignoring key_sends entry {chord:?}: {e}"),
            }
        }
        out
    }
}

/// Expand backslash escapes in a `key_sends` value into raw bytes.
pub fn parse_escapes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('e') => out.push(0x1b),
            Some('n') => out.push(b'\n'),
            Some('r') => out.push(b'\r'),
            Some('t') => out.push(b'\t'),
            Some('0') => out.push(0),
            Some('\\') => out.push(b'\\'),
            Some('x') => {
                let hex: String = (0..2).filter_map(|_| chars.next()).collect();
                match u8::from_str_radix(&hex, 16) {
                    Ok(b) => out.push(b),
                    Err(_) => {
                        out.push(b'\\');
                        out.push(b'x');
                        out.extend_from_slice(hex.as_bytes());
                    }
                }
            }
            Some('u') => {
                // \u{NNNN}
                let rest: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let hex = rest.strip_prefix('{').unwrap_or(&rest);
                match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => {
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    None => {
                        out.push(b'\\');
                        out.push(b'u');
                        out.extend_from_slice(rest.as_bytes());
                    }
                }
            }
            Some(other) => {
                out.push(b'\\');
                let mut buf = [0u8; 4];
                out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
            }
            None => out.push(b'\\'),
        }
    }
    out
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
    fn pretty_chord_reads_naturally() {
        assert_eq!(pretty_chord("ctrl+shift+t"), "Ctrl+Shift+T");
        assert_eq!(pretty_chord("ctrl+w"), "Ctrl+W");
        assert_eq!(pretty_chord("ctrl+plus"), "Ctrl++");
        assert_eq!(pretty_chord("shift+pageup"), "Shift+PgUp");
        assert_eq!(pretty_chord("ctrl+shift+pagedown"), "Ctrl+Shift+PgDn");
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
    fn actions_round_trip_as_strings() {
        for a in [
            Action::NewTab,
            Action::ReopenTab,
            Action::TabSearch,
            Action::Omnibox,
            Action::FontIncrease,
            Action::GotoTab(3),
            Action::WindowMaximize,
            Action::WindowLeftHalf,
            Action::OpenScrollback,
            Action::Disabled,
        ] {
            let json = serde_json::to_string(&a).unwrap();
            assert!(json.starts_with('"'), "{a:?} should serialize as a string");
            assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), a);
        }
        assert_eq!(
            serde_json::to_string(&Action::GotoTab(3)).unwrap(),
            r#""goto-tab-3""#
        );
    }

    #[test]
    fn tab_actions_are_active() {
        assert!(Action::NewTab.is_active());
        assert!(Action::NextTab.is_active());
        assert!(Action::GotoTab(2).is_active());
        assert!(Action::HistorySearch.is_active());
        assert!(!Action::Disabled.is_active());
    }

    #[test]
    fn goto_tab_bindings_exist_by_default() {
        let compiled = KeyBindings::default().compile();
        for n in 1..=9u8 {
            assert!(
                compiled.iter().any(|(_, a)| *a == Action::GotoTab(n)),
                "missing default binding for goto-tab {n}"
            );
        }
    }

    #[test]
    fn partial_config_keeps_defaults() {
        // A config that only rebinds one thing still gets every default.
        let kb: KeyBindings = serde_json::from_str(r#"{ "ctrl+shift+e": "new-tab" }"#).unwrap();
        let merged = kb.merged();
        assert_eq!(merged.get("ctrl+shift+e"), Some(&Action::NewTab));
        assert_eq!(merged.get("ctrl+1"), Some(&Action::GotoTab(1)));
        assert_eq!(merged.get("ctrl+shift+t"), Some(&Action::ReopenTab));
    }

    #[test]
    fn override_and_disable() {
        let kb: KeyBindings =
            serde_json::from_str(r#"{ "ctrl+shift+t": "clear", "ctrl+r": "disabled" }"#).unwrap();
        let merged = kb.merged();
        assert_eq!(merged.get("ctrl+shift+t"), Some(&Action::Clear));
        assert_eq!(merged.get("ctrl+r"), None);
    }

    #[test]
    fn plus_and_equals_are_distinct_keys() {
        assert_eq!(parse_chord("ctrl+equals").unwrap().key, Key::Equals);
        assert_eq!(parse_chord("ctrl+plus").unwrap().key, Key::Plus);
        assert_ne!(
            parse_chord("ctrl+equals").unwrap().key,
            parse_chord("ctrl+plus").unwrap().key
        );
    }

    #[test]
    fn disabling_an_unknown_chord_is_harmless() {
        let kb: KeyBindings = serde_json::from_str(r#"{ "ctrl+shift+f19": "disabled" }"#).unwrap();
        // Every default is still present, and nothing panics.
        assert_eq!(kb.merged().len(), KeyBindings::default().0.len());
    }

    #[test]
    fn compile_drops_unparseable_chords_but_keeps_the_rest() {
        let kb: KeyBindings =
            serde_json::from_str(r#"{ "ctrl+nonsense": "new-tab", "ctrl+shift+e": "clear" }"#)
                .unwrap();
        let compiled = kb.compile();
        // The bogus chord is skipped; the good one and all defaults remain.
        assert_eq!(compiled.len(), KeyBindings::default().0.len() + 1);
        assert!(compiled.iter().all(|(_, a)| *a != Action::Disabled));
    }

    #[test]
    fn parse_escapes_expands_the_common_forms() {
        assert_eq!(parse_escapes("b"), b"b");
        assert_eq!(parse_escapes(r"\e[1;5D"), b"\x1b[1;5D");
        assert_eq!(parse_escapes(r"\x1b\x5b"), b"\x1b[");
        assert_eq!(parse_escapes(r"\r\n\t\0"), b"\r\n\t\0");
        assert_eq!(parse_escapes(r"a\\b"), b"a\\b");
        assert_eq!(parse_escapes(r"\u{1b}OP"), b"\x1bOP");
        // A malformed escape is kept literally rather than dropped.
        assert_eq!(parse_escapes(r"\q"), b"\\q");
    }

    #[test]
    fn key_sends_compile_skips_bad_chords() {
        let ks: KeySends =
            serde_json::from_str(r#"{ "alt+left": "b", "ctrl+nonsense": "\\u{1b}x" }"#).unwrap();
        let compiled = ks.compile();
        assert_eq!(compiled.len(), 1);
        assert_eq!(compiled[0].0.key, Key::ArrowLeft);
        assert_eq!(compiled[0].1, b"b");
    }

    #[test]
    fn every_compiled_chord_is_unique() {
        let compiled = KeyBindings::default().compile();
        let mut seen = std::collections::HashSet::new();
        for (chord, _) in &compiled {
            assert!(
                seen.insert(format!("{chord:?}")),
                "duplicate compiled chord: {chord:?}"
            );
        }
    }
}
