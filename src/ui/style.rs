//! The chrome design system: one palette-derived set of surface shades, and the
//! full `egui::Style` built from it. Keeping this in one place means the tab
//! strip, menus, modals and future overlays all read as the same material.

use egui::{Color32, CornerRadius, Margin, Shadow, Stroke, TextStyle, Visuals, vec2};

use crate::config::ThemeConfig;
use crate::config::theme::parse_hex;

/// Shades derived from the active palette. All blends are in sRGB — good enough
/// for chrome, and it means a light palette produces darker surfaces the same
/// way a dark one produces lighter ones.
#[derive(Debug, Clone, Copy)]
pub struct Surfaces {
    /// Window / terminal background.
    pub bg: Color32,
    /// Panels sitting above `bg` (the tab strip).
    pub raised: Color32,
    /// Hover fill for interactive chrome.
    pub hover: Color32,
    /// Pressed / selected fill.
    pub pressed: Color32,
    /// Hairline borders and separators.
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    /// Non-zero-exit / destructive accent (the close `×` halo; block gutters later).
    pub err: Color32,
}

impl Surfaces {
    pub fn from_theme(theme: &ThemeConfig) -> Self {
        let p = theme.palette();
        let bg = hex(&p.background);
        let fg = hex(&p.foreground);
        Self {
            bg,
            raised: mix(bg, fg, 0.055),
            hover: mix(bg, fg, 0.11),
            pressed: mix(bg, fg, 0.17),
            border: mix(bg, fg, 0.20),
            text: fg,
            text_muted: mix(fg, bg, 0.38),
            text_faint: mix(fg, bg, 0.60),
            accent: hex(&p.blue),
            err: hex(&p.red),
        }
    }
}

/// Build the full `egui::Style` for `theme` and install it on `ctx`. Replaces
/// the previous four-field `Visuals` tweak; safe to call again on config reload.
/// The config's theme wins regardless of the OS light/dark preference, so the
/// same style is written for both egui themes.
pub fn apply(ctx: &egui::Context, theme: &ThemeConfig) {
    let s = Surfaces::from_theme(theme);
    let dark = theme.is_dark();
    let warn = hex(&theme.palette().yellow);
    let visuals = build_visuals(&s, dark, warn);

    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(11.0, 5.0);
        style.spacing.window_margin = Margin::same(12);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.interact_size.y = 26.0;

        for (text_style, size) in [
            (TextStyle::Body, 13.5_f32),
            (TextStyle::Button, 13.5),
            (TextStyle::Small, 11.5),
        ] {
            if let Some(font_id) = style.text_styles.get_mut(&text_style) {
                font_id.size = size;
            }
        }
    });
}

fn build_visuals(s: &Surfaces, dark: bool, warn: Color32) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    v.panel_fill = s.bg;
    v.window_fill = s.raised;
    v.extreme_bg_color = s.bg;
    v.faint_bg_color = s.hover;
    v.override_text_color = Some(s.text);
    v.hyperlink_color = s.accent;
    v.warn_fg_color = warn;
    v.error_fg_color = s.err;

    v.selection.bg_fill = s.accent.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, s.text);

    v.window_stroke = Stroke::new(1.0, s.border);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    let alpha = |dark_a: u8, light_a: u8| if dark { dark_a } else { light_a };
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(alpha(120, 45)),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(alpha(90, 30)),
    };

    let radius = CornerRadius::same(6);
    let w = &mut v.widgets;

    w.noninteractive.bg_fill = s.raised;
    w.noninteractive.weak_bg_fill = s.raised;
    w.noninteractive.bg_stroke = Stroke::new(1.0, s.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, s.text_muted);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = s.hover;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, s.text);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = s.pressed;
    w.hovered.weak_bg_fill = s.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, s.border);
    w.hovered.fg_stroke = Stroke::new(1.0, s.text);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 1.0;

    w.active.bg_fill = s.accent;
    w.active.weak_bg_fill = s.pressed;
    w.active.bg_stroke = Stroke::new(1.0, s.accent);
    w.active.fg_stroke = Stroke::new(1.0, on_accent(s.accent));
    w.active.corner_radius = radius;
    w.active.expansion = 1.0;

    w.open = w.hovered;

    v
}

/// A readable text/icon color to sit on top of a saturated fill (accent button,
/// danger button, block gutter label).
pub fn on_accent(accent: Color32) -> Color32 {
    let [r, g, b, _] = accent.to_array();
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if luma > 150.0 {
        Color32::from_rgb(20, 20, 20)
    } else {
        Color32::WHITE
    }
}

fn hex(s: &str) -> Color32 {
    let [r, g, b] = parse_hex(s).unwrap_or([128, 128, 128]);
    Color32::from_rgb(r, g, b)
}

/// Linear blend in sRGB space: `t = 0` returns `a`, `t = 1` returns `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_endpoints_and_midpoint() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(100, 200, 40);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color32::from_rgb(50, 100, 20));
        assert_eq!(mix(a, b, -1.0), a);
        assert_eq!(mix(a, b, 2.0), b);
    }

    #[test]
    fn surfaces_are_ordered_away_from_bg() {
        // Dark default: raised < hover < pressed < border in "distance from bg".
        let s = Surfaces::from_theme(&ThemeConfig::default());
        let d = |c: Color32| {
            (c.r() as i32 - s.bg.r() as i32).abs()
                + (c.g() as i32 - s.bg.g() as i32).abs()
                + (c.b() as i32 - s.bg.b() as i32).abs()
        };
        assert!(d(s.raised) < d(s.hover));
        assert!(d(s.hover) < d(s.pressed));
        assert!(d(s.pressed) <= d(s.border));
        assert_eq!(s.text, mix(s.text, s.bg, 0.0));
    }

    #[test]
    fn on_accent_contrasts() {
        assert_eq!(
            on_accent(Color32::from_rgb(240, 240, 200)),
            Color32::from_rgb(20, 20, 20)
        );
        assert_eq!(on_accent(Color32::from_rgb(30, 60, 120)), Color32::WHITE);
    }
}
