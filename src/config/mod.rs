//! JSON configuration: load, create-with-defaults, and live-reload.

pub mod keybindings;
pub mod theme;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use keybindings::KeyBindings;
pub use theme::ThemeConfig;

/// The whole user configuration, deserialized from `config.json`.
///
/// Every field has a default so a partial (or empty `{}`) config file still
/// works; unknown fields are ignored rather than rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Font to use: either a path to a `.ttf`/`.otf`/`.ttc` file, or the name of
    /// an installed font family (matched case-insensitively via the system font
    /// directories). `None`, or a value that resolves to nothing, uses the
    /// bundled monospace face.
    pub font_family: Option<String>,
    /// Terminal font size in points.
    pub font_size: f32,
    /// Scrollback buffer size, in lines. (Reserved: wired to the backend in a
    /// later milestone.)
    pub scrollback_lines: u32,
    /// Shell to spawn. `None` uses `$SHELL` (falling back to `/bin/bash`).
    pub shell: Option<String>,
    /// Extra arguments passed to the shell.
    pub shell_args: Vec<String>,
    /// Color theme: either an inline palette object (how the generated config
    /// writes it — Solarized Dark, fully expanded and editable) or a builtin
    /// name (`"solarized-dark"`, `"solarized-light"`, `"default-dark"`,
    /// `"default-light"`).
    pub theme: ThemeConfig,
    /// Keyboard shortcuts, mapping a chord string (`"ctrl+shift+t"`) to an
    /// [`Action`].
    pub keybindings: KeyBindings,
    /// Initial window size, and how the window frame is drawn.
    pub window: WindowConfig,
    /// The `Ctrl+R` command-history search.
    pub history: HistoryConfig,
    /// Reopening tabs from the last run on startup.
    pub session: SessionConfig,
    /// Cursor shape and blink, before any app overrides it (e.g. via DECSCUSR).
    pub cursor: CursorConfig,
    /// Clipboard-paste behaviour.
    pub paste: PasteConfig,
    /// Desktop notifications.
    pub notifications: NotificationsConfig,
    /// Safety indicators.
    pub security: SecurityConfig,
}

/// Safety indicators.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Outline the terminal in red and mark the tab when the shell (or
    /// something under it — `sudo -s`, `su`, a long `sudo …`) is running as
    /// `root`. `true` by default. Linux only.
    pub superuser_warning: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            superuser_warning: true,
        }
    }
}

/// Desktop notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationsConfig {
    /// Post a desktop notification when a command that ran at least this many
    /// seconds finishes while pomptty is unfocused or on another tab. `0`
    /// disables it. Default 300 (5 minutes). Needs the shell-integration hook.
    pub long_command_secs: u64,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            long_command_secs: 300,
        }
    }
}

/// Paste safety.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PasteConfig {
    /// Confirm before pasting text that contains a newline into a shell that
    /// hasn't turned on bracketed paste — where each line would run
    /// immediately. `true` by default.
    pub confirm_multiline: bool,
}

impl Default for PasteConfig {
    fn default() -> Self {
        Self {
            confirm_multiline: true,
        }
    }
}

/// The cursor's default shape and blink. An app that sets its own cursor style
/// at runtime (vim's insert-mode beam, say) overrides this — it's only what's
/// shown before anything has.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CursorConfig {
    pub shape: CursorShapeConfig,
    pub blink: bool,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            shape: CursorShapeConfig::default(),
            blink: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShapeConfig {
    #[default]
    Block,
    Beam,
    Underline,
}

impl CursorShapeConfig {
    pub fn to_egui_term(self) -> egui_term::CursorShape {
        match self {
            CursorShapeConfig::Block => egui_term::CursorShape::Block,
            CursorShapeConfig::Beam => egui_term::CursorShape::Beam,
            CursorShapeConfig::Underline => egui_term::CursorShape::Underline,
        }
    }
}

/// Session restore: remember the open tabs (directory + any rename) across
/// quits and crashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    /// When `false`, pomptty neither records nor restores the open tabs —
    /// every launch starts with a single tab in pomptty's own directory.
    pub restore: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self { restore: true }
    }
}

/// The `Ctrl+R` fuzzy history search (fed by the `pomptty --print-integration`
/// shell hook).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    /// When `false`, `Ctrl+R` is handed to the shell's own reverse-i-search
    /// instead of opening pomptty's overlay.
    pub enabled: bool,
    /// Most results to show in the overlay at once.
    pub max_results: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Initial window size in logical pixels.
    pub width: f32,
    pub height: f32,
    /// `"custom"` (default) drops the OS title bar and makes pomptty's own tab
    /// strip the title bar, Chrome-style. `"system"` keeps the OS title bar —
    /// use it if your window manager handles a borderless window poorly (no drop
    /// shadow on non-compositing X11). Takes effect on restart.
    pub decorations: Decoration,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 900.0,
            height: 560.0,
            decorations: Decoration::default(),
        }
    }
}

/// How the window frame is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decoration {
    /// Frameless: pomptty draws its own title bar / borders.
    #[default]
    Custom,
    /// The OS-drawn title bar and borders.
    System,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 14.0,
            scrollback_lines: 10_000,
            shell: None,
            shell_args: Vec::new(),
            theme: ThemeConfig::default(),
            keybindings: KeyBindings::default(),
            window: WindowConfig::default(),
            history: HistoryConfig::default(),
            session: SessionConfig::default(),
            cursor: CursorConfig::default(),
            paste: PasteConfig::default(),
            notifications: NotificationsConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Config {
    /// Directory that holds `config.json` (`~/.config/pomptty` on Linux).
    pub fn config_dir() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "pomptty")
            .context("could not determine a config directory for this platform")?;
        Ok(dirs.config_dir().to_path_buf())
    }

    /// Full path to `config.json`.
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Load the config from `path`, or create it with defaults if it is missing.
    ///
    /// A malformed file is not overwritten: the error is returned so the caller
    /// can surface it and keep running on the previous (or default) config.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let cfg: Config = serde_json::from_str(&text)
                    .with_context(|| format!("failed to parse {}", path.display()))?;
                Ok(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Config::default();
                cfg.save(path).with_context(|| {
                    format!("failed to write default config to {}", path.display())
                })?;
                log::info!("wrote a default config to {}", path.display());
                Ok(cfg)
            }
            Err(e) => Err(e).with_context(|| format!("failed to read {}", path.display())),
        }
    }

    /// Serialize the config to `path` (pretty-printed), creating parent dirs.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_object_is_valid_and_matches_default() {
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.font_size, Config::default().font_size);
        assert!(cfg.keybindings.get("ctrl+minus").is_some());
    }

    #[test]
    fn partial_config_fills_gaps() {
        let cfg: Config = serde_json::from_str(r#"{ "font_size": 18.0 }"#).unwrap();
        assert_eq!(cfg.font_size, 18.0);
        assert_eq!(cfg.scrollback_lines, Config::default().scrollback_lines);
    }

    #[test]
    fn roundtrips_through_json() {
        let cfg = Config::default();
        let text = serde_json::to_string_pretty(&cfg).unwrap();
        let back: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(back.font_size, cfg.font_size);
    }

    #[test]
    fn inline_theme_parses() {
        let cfg: Config = serde_json::from_str(
            r##"{ "theme": { "background": "#000000", "foreground": "#ffffff" } }"##,
        )
        .unwrap();
        assert!(matches!(cfg.theme, ThemeConfig::Custom(_)));
    }

    #[test]
    fn default_config_writes_theme_as_a_full_palette() {
        // The generated config should show every color inline, not the name.
        let json = serde_json::to_value(Config::default()).unwrap();
        let theme = &json["theme"];
        assert!(theme.is_object(), "theme should serialize as an object");
        assert_eq!(theme["background"], "#002b36");
        for key in ["foreground", "red", "green", "blue", "bright_white"] {
            assert!(theme.get(key).is_some(), "missing color {key}");
        }
    }

    #[test]
    fn named_theme_still_accepted() {
        let cfg: Config = serde_json::from_str(r#"{ "theme": "solarized-light" }"#).unwrap();
        assert!(!cfg.theme.is_dark());
    }

    #[test]
    fn missing_file_is_created_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.json");
        assert!(!path.exists());

        let cfg = Config::load_or_create(&path).unwrap();
        assert!(path.exists(), "the default config should have been written");
        assert_eq!(cfg.font_size, Config::default().font_size);

        // Re-loading the file we just wrote yields the same config.
        let again = Config::load_or_create(&path).unwrap();
        assert_eq!(again.font_size, cfg.font_size);
    }

    #[test]
    fn custom_keybinding_survives_a_save_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let mut cfg = Config::default();
        cfg.keybindings
            .0
            .insert("ctrl+shift+e".into(), keybindings::Action::NewTab);
        cfg.keybindings
            .0
            .insert("ctrl+r".into(), keybindings::Action::Disabled);
        cfg.save(&path).unwrap();

        let loaded = Config::load_or_create(&path).unwrap();
        let merged = loaded.keybindings.merged();
        assert_eq!(
            merged.get("ctrl+shift+e"),
            Some(&keybindings::Action::NewTab)
        );
        assert_eq!(merged.get("ctrl+r"), None, "ctrl+r was disabled");
        // A default the user didn't touch is still there.
        assert_eq!(
            merged.get("ctrl+shift+t"),
            Some(&keybindings::Action::ReopenTab)
        );
    }

    #[test]
    fn malformed_file_errors_and_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let garbage = "{ this is not valid json ";
        std::fs::write(&path, garbage).unwrap();

        let err = Config::load_or_create(&path);
        assert!(err.is_err(), "a malformed config must not load");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            garbage,
            "the malformed file must not be overwritten"
        );
    }
}
