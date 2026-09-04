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
    /// Initial window size in logical pixels.
    pub window: WindowConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 900.0,
            height: 560.0,
        }
    }
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
            Some(&keybindings::Action::NewTab)
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
