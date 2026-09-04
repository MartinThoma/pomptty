//! Color themes: builtin palettes by name, or an inline palette in the config.

use egui_term::{ColorPalette, TerminalTheme};
use serde::{Deserialize, Serialize};

/// A theme is either a builtin name or a full inline palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeConfig {
    Named(String),
    Custom(Box<PaletteConfig>),
}

impl Default for ThemeConfig {
    /// The default is the Solarized Dark palette written out *in full*, so a
    /// freshly generated `config.json` shows every color and invites editing
    /// (rather than hiding them behind the name `"solarized-dark"`).
    fn default() -> Self {
        ThemeConfig::Custom(Box::new(PaletteConfig::solarized_dark()))
    }
}

impl ThemeConfig {
    /// Resolve to a concrete palette, falling back to `solarized-dark` for an
    /// unknown builtin name (with a warning).
    pub fn palette(&self) -> PaletteConfig {
        match self {
            ThemeConfig::Custom(p) => (**p).clone(),
            ThemeConfig::Named(name) => match name.as_str() {
                "solarized-dark" => PaletteConfig::solarized_dark(),
                "solarized-light" => PaletteConfig::solarized_light(),
                "default-light" => PaletteConfig::default_light(),
                "default-dark" => PaletteConfig::default_dark(),
                other => {
                    log::warn!("unknown theme {other:?}, using \"solarized-dark\"");
                    PaletteConfig::solarized_dark()
                }
            },
        }
    }

    /// Build the `egui_term` terminal theme for this config.
    pub fn terminal_theme(&self) -> TerminalTheme {
        TerminalTheme::new(Box::new(self.palette().into()))
    }

    /// Whether the resolved background is dark. Drives light/dark base `Visuals`
    /// and shade derivation in [`crate::ui::style`].
    pub fn is_dark(&self) -> bool {
        let [r, g, b] = parse_hex(&self.palette().background).unwrap_or([0, 0, 0]);
        // Rec. 601 luma.
        (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) < 128.0
    }
}

/// The 18 user-facing colors of a palette. All fields default to the dark
/// theme's value so a partial inline theme still resolves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PaletteConfig {
    pub foreground: String,
    pub background: String,
    pub cursor: Option<String>,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

impl Default for PaletteConfig {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl PaletteConfig {
    /// Solarized Dark (Ethan Schoonover), the default theme.
    fn solarized_dark() -> Self {
        Self {
            foreground: "#839496".into(), // base0
            background: "#002b36".into(), // base03
            cursor: Some("#93a1a1".into()),
            black: "#073642".into(),          // base02
            red: "#dc322f".into(),            // red
            green: "#859900".into(),          // green
            yellow: "#b58900".into(),         // yellow
            blue: "#268bd2".into(),           // blue
            magenta: "#d33682".into(),        // magenta
            cyan: "#2aa198".into(),           // cyan
            white: "#eee8d5".into(),          // base2
            bright_black: "#002b36".into(),   // base03
            bright_red: "#cb4b16".into(),     // orange
            bright_green: "#586e75".into(),   // base01
            bright_yellow: "#657b83".into(),  // base00
            bright_blue: "#839496".into(),    // base0
            bright_magenta: "#6c71c4".into(), // violet
            bright_cyan: "#93a1a1".into(),    // base1
            bright_white: "#fdf6e3".into(),   // base3
        }
    }

    /// Solarized Light (Ethan Schoonover).
    fn solarized_light() -> Self {
        Self {
            foreground: "#657b83".into(), // base00
            background: "#fdf6e3".into(), // base3
            cursor: Some("#586e75".into()),
            black: "#073642".into(),
            red: "#dc322f".into(),
            green: "#859900".into(),
            yellow: "#b58900".into(),
            blue: "#268bd2".into(),
            magenta: "#d33682".into(),
            cyan: "#2aa198".into(),
            white: "#eee8d5".into(),
            bright_black: "#002b36".into(),
            bright_red: "#cb4b16".into(),
            bright_green: "#586e75".into(),
            bright_yellow: "#657b83".into(),
            bright_blue: "#839496".into(),
            bright_magenta: "#6c71c4".into(),
            bright_cyan: "#93a1a1".into(),
            bright_white: "#fdf6e3".into(),
        }
    }

    fn default_dark() -> Self {
        Self {
            foreground: "#d8d8d8".into(),
            background: "#181818".into(),
            cursor: None,
            black: "#181818".into(),
            red: "#ac4242".into(),
            green: "#90a959".into(),
            yellow: "#f4bf75".into(),
            blue: "#6a9fb5".into(),
            magenta: "#aa759f".into(),
            cyan: "#75b5aa".into(),
            white: "#d8d8d8".into(),
            bright_black: "#6b6b6b".into(),
            bright_red: "#c55555".into(),
            bright_green: "#aac474".into(),
            bright_yellow: "#feca88".into(),
            bright_blue: "#82b8c8".into(),
            bright_magenta: "#c28cb8".into(),
            bright_cyan: "#93d3c3".into(),
            bright_white: "#f8f8f8".into(),
        }
    }

    fn default_light() -> Self {
        Self {
            foreground: "#2d2d2d".into(),
            background: "#f7f7f7".into(),
            cursor: None,
            black: "#2d2d2d".into(),
            red: "#c0392b".into(),
            green: "#27ae60".into(),
            yellow: "#b9770e".into(),
            blue: "#2471a3".into(),
            magenta: "#8e44ad".into(),
            cyan: "#17a589".into(),
            white: "#e6e6e6".into(),
            bright_black: "#5c5c5c".into(),
            bright_red: "#e74c3c".into(),
            bright_green: "#2ecc71".into(),
            bright_yellow: "#f39c12".into(),
            bright_blue: "#3498db".into(),
            bright_magenta: "#9b59b6".into(),
            bright_cyan: "#1abc9c".into(),
            bright_white: "#ffffff".into(),
        }
    }
}

impl From<PaletteConfig> for ColorPalette {
    fn from(p: PaletteConfig) -> Self {
        // `egui_term::ColorPalette` also carries `dim_*` entries; we leave those
        // at their defaults (rarely exercised by real programs) and only map the
        // 18 standard colors plus fg/bg.
        ColorPalette {
            foreground: norm_hex(&p.foreground),
            background: norm_hex(&p.background),
            black: norm_hex(&p.black),
            red: norm_hex(&p.red),
            green: norm_hex(&p.green),
            yellow: norm_hex(&p.yellow),
            blue: norm_hex(&p.blue),
            magenta: norm_hex(&p.magenta),
            cyan: norm_hex(&p.cyan),
            white: norm_hex(&p.white),
            bright_black: norm_hex(&p.bright_black),
            bright_red: norm_hex(&p.bright_red),
            bright_green: norm_hex(&p.bright_green),
            bright_yellow: norm_hex(&p.bright_yellow),
            bright_blue: norm_hex(&p.bright_blue),
            bright_magenta: norm_hex(&p.bright_magenta),
            bright_cyan: norm_hex(&p.bright_cyan),
            bright_white: norm_hex(&p.bright_white),
            bright_foreground: None,
            ..ColorPalette::default()
        }
    }
}

/// Parse `#rrggbb` (or `rrggbb`) into bytes.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}

/// Normalize a hex color to the `#rrggbb` form `egui_term` expects, defaulting
/// to black on anything unparseable (so a typo can't panic the renderer).
fn norm_hex(s: &str) -> String {
    match parse_hex(s) {
        Some([r, g, b]) => format!("#{r:02x}{g:02x}{b:02x}"),
        None => {
            log::warn!("invalid color {s:?}, using #000000");
            "#000000".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_builtins_resolve() {
        assert!(ThemeConfig::Named("solarized-dark".into()).is_dark());
        assert!(!ThemeConfig::Named("solarized-light".into()).is_dark());
        assert!(ThemeConfig::Named("default-dark".into()).is_dark());
        assert!(!ThemeConfig::Named("default-light".into()).is_dark());
    }

    #[test]
    fn default_theme_is_solarized_dark() {
        assert_eq!(ThemeConfig::default().palette().background, "#002b36");
    }

    #[test]
    fn unknown_name_falls_back_to_default() {
        let t = ThemeConfig::Named("nope".into());
        assert_eq!(
            t.palette().background,
            PaletteConfig::solarized_dark().background
        );
    }

    #[test]
    fn inline_partial_theme_keeps_defaults() {
        let t: ThemeConfig = serde_json::from_str(r##"{ "background": "#101010" }"##).unwrap();
        let p = t.palette();
        assert_eq!(p.background, "#101010");
        assert_eq!(p.foreground, PaletteConfig::default_dark().foreground);
    }

    #[test]
    fn bad_hex_normalizes_without_panic() {
        assert_eq!(norm_hex("oops"), "#000000");
        assert_eq!(norm_hex("#ABCDEF"), "#abcdef");
    }
}
