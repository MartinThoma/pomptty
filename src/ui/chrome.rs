//! The top bar: a minimal, Chrome-flavored tab strip.

use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Vec2, pos2, vec2};

/// What the user asked the chrome to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeAction {
    None,
    NewTab,
    SelectTab(usize),
    CloseTab(usize),
    /// The `Ctrl+R`-style search affordance was clicked. (Inert until the
    /// history milestone.)
    OpenSearch,
}

pub struct TabStrip<'a> {
    pub titles: &'a [String],
    pub active: usize,
    /// Terminal foreground / background — the strip is drawn as shades of these
    /// so it reads as one surface with the grid below.
    pub fg: Color32,
    pub bg: Color32,
    pub accent: Color32,
}

const TAB_MIN_WIDTH: f32 = 44.0;
const TAB_MAX_WIDTH: f32 = 220.0;
const TAB_HEIGHT: f32 = 28.0;
const CLOSE_SIZE: f32 = 16.0;
const BTN_SIZE: f32 = 22.0;
const TAB_ROUNDING: u8 = 7;

impl TabStrip<'_> {
    pub fn show(self, ui: &mut egui::Ui) -> ChromeAction {
        let mut action = ChromeAction::None;

        let muted = mix(self.fg, self.bg, 0.55);
        let strip_fill = mix(self.bg, self.fg, 0.05);
        let baseline = mix(self.bg, self.fg, 0.15);

        let frame = egui::Frame::new()
            .fill(strip_fill)
            .inner_margin(egui::Margin {
                left: 6,
                right: 6,
                top: 4,
                bottom: 0,
            });

        egui::Panel::top("pomptty_tab_strip")
            .frame(frame)
            .resizable(false)
            .show_separator_line(false)
            .show(ui, |ui| {
                let icon_rest = mix(self.fg, self.bg, 0.7);
                let mut active_rect = None;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(3.0, 0.0);

                    // Space kept on the right for `+` and the search button.
                    let reserve = BTN_SIZE + 3.0 + BTN_SIZE + 8.0;
                    egui::ScrollArea::horizontal()
                        .max_width((ui.available_width() - reserve).max(0.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
                                let w = tab_width(ui.available_width(), self.titles.len());
                                for (i, title) in self.titles.iter().enumerate() {
                                    let r = self.tab(ui, i, title, w, muted, &mut action);
                                    if i == self.active {
                                        active_rect = Some(r);
                                    }
                                }
                                if plus_button(ui, icon_rest, self.fg)
                                    .on_hover_text("New tab  (Ctrl+Shift+T)")
                                    .clicked()
                                {
                                    action = ChromeAction::NewTab;
                                }
                            });
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if search_button(ui, icon_rest, self.fg)
                            .on_hover_text("Search history (Ctrl+R) — coming soon")
                            .clicked()
                        {
                            action = ChromeAction::OpenSearch;
                        }
                    });
                });

                let rect = ui.max_rect();
                let y = rect.bottom() - 0.5;
                ui.painter()
                    .hline(rect.left()..=rect.right(), y, Stroke::new(1.0, baseline));
                // Let the active tab sit flush against the grid below it.
                if let Some(r) = active_rect {
                    ui.painter()
                        .hline(r.left()..=r.right(), y, Stroke::new(1.5, self.bg));
                }
            });

        action
    }

    fn tab(
        &self,
        ui: &mut egui::Ui,
        i: usize,
        title: &str,
        width: f32,
        muted: Color32,
        action: &mut ChromeAction,
    ) -> Rect {
        let selected = i == self.active;
        let (rect, resp) = ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::click());
        let show_close = selected || resp.hovered() || self.titles.len() > 1;

        let fill = if selected {
            self.bg
        } else if resp.hovered() {
            mix(self.bg, self.fg, 0.10)
        } else {
            Color32::TRANSPARENT
        };
        let rounding = CornerRadius {
            nw: TAB_ROUNDING,
            ne: TAB_ROUNDING,
            sw: 0,
            se: 0,
        };
        {
            let p = ui.painter();
            p.rect_filled(rect, rounding, fill);
            if selected {
                p.hline(
                    rect.left() + 1.0..=rect.right() - 1.0,
                    rect.top() + 1.0,
                    Stroke::new(2.0, self.accent),
                );
                // Punch through the strip baseline under the active tab.
                p.hline(
                    rect.left()..=rect.right(),
                    rect.bottom(),
                    Stroke::new(1.5, self.bg),
                );
            }
        }

        // Close button (right-aligned inside the tab).
        let close_rect = Rect::from_center_size(
            pos2(rect.right() - 5.0 - CLOSE_SIZE / 2.0, rect.center().y),
            Vec2::splat(CLOSE_SIZE),
        );
        let close = ui.interact(close_rect, resp.id.with("close"), Sense::click());
        let text_color = if selected { self.fg } else { muted };
        if show_close {
            paint_close(ui, close_rect, &close, text_color, self.fg);
        }

        // Title, clipped to the room left of the close button.
        let clip_right = if show_close {
            close_rect.left() - 4.0
        } else {
            rect.right() - 8.0
        };
        let text_clip = Rect::from_min_max(
            pos2(rect.left() + 10.0, rect.top()),
            pos2(clip_right, rect.bottom()),
        );
        if text_clip.width() > 1.0 {
            let budget = (text_clip.width() / 6.6).floor().max(1.0) as usize;
            let galley = ui.painter().layout_no_wrap(
                truncate(title, budget),
                egui::FontId::proportional(13.0),
                text_color,
            );
            let pos = pos2(text_clip.left(), rect.center().y - galley.size().y / 2.0);
            ui.painter()
                .with_clip_rect(text_clip)
                .galley(pos, galley, text_color);
        }

        resp.clone().on_hover_text(title);

        // A click that lands on the close glyph closes regardless of which
        // overlapping widget egui hands it to.
        let on_close = ui
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| close_rect.contains(p));
        if close.clicked()
            || (resp.clicked() && on_close)
            || resp.clicked_by(egui::PointerButton::Middle)
        {
            *action = ChromeAction::CloseTab(i);
        } else if resp.clicked() {
            *action = ChromeAction::SelectTab(i);
        }

        rect
    }
}

/// Width for each tab: share the available space, Chrome-style, clamped.
fn tab_width(available: f32, count: usize) -> f32 {
    if count == 0 {
        return TAB_MAX_WIDTH;
    }
    let each = available / count as f32 - 3.0;
    each.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
}

/// Truncate to `max` chars on a char boundary, appending `…`.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    if max <= 1 {
        return "…".to_owned();
    }
    let keep: String = s.chars().take(max - 1).collect();
    format!("{keep}…")
}

/// `×` drawn as strokes (no font dependency), with a hover halo.
fn paint_close(ui: &egui::Ui, rect: Rect, resp: &egui::Response, rest: Color32, active: Color32) {
    let p = ui.painter();
    if resp.hovered() {
        p.circle_filled(rect.center(), CLOSE_SIZE / 2.0, active.gamma_multiply(0.22));
    }
    let color = if resp.hovered() { active } else { rest };
    let r = 3.0;
    let c = rect.center();
    let stroke = Stroke::new(1.4, color);
    p.line_segment([c + vec2(-r, -r), c + vec2(r, r)], stroke);
    p.line_segment([c + vec2(-r, r), c + vec2(r, -r)], stroke);
}

fn plus_button(ui: &mut egui::Ui, rest: Color32, active: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BTN_SIZE), Sense::click());
    let hot = resp.hovered();
    if hot {
        ui.painter()
            .rect_filled(rect, 5, active.gamma_multiply(0.15));
    }
    let color = if hot { active } else { rest };
    let c = rect.center();
    let r = 4.0;
    let stroke = Stroke::new(1.5, color);
    ui.painter()
        .line_segment([c + vec2(-r, 0.0), c + vec2(r, 0.0)], stroke);
    ui.painter()
        .line_segment([c + vec2(0.0, -r), c + vec2(0.0, r)], stroke);
    resp
}

fn search_button(ui: &mut egui::Ui, rest: Color32, active: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BTN_SIZE), Sense::click());
    let hot = resp.hovered();
    if hot {
        ui.painter()
            .rect_filled(rect, 5, active.gamma_multiply(0.15));
    }
    let color = if hot { active } else { rest };
    let stroke = Stroke::new(1.4, color);
    let c = rect.center() - vec2(1.0, 1.0);
    ui.painter().circle_stroke(c, 4.0, stroke);
    let a = c + vec2(2.8, 2.8);
    ui.painter().line_segment([a, a + vec2(3.5, 3.5)], stroke);
    resp
}

/// Linear blend in sRGB space: `t = 0` returns `a`, `t = 1` returns `b`.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::{Color32, TAB_MAX_WIDTH, TAB_MIN_WIDTH, mix, tab_width, truncate};

    #[test]
    fn mix_endpoints_and_midpoint() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(100, 200, 40);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color32::from_rgb(50, 100, 20));
    }

    #[test]
    fn tab_width_is_clamped() {
        assert_eq!(tab_width(2000.0, 1), TAB_MAX_WIDTH);
        assert_eq!(tab_width(100.0, 20), TAB_MIN_WIDTH);
        let mid = tab_width(400.0, 3);
        assert!(mid > TAB_MIN_WIDTH && mid < TAB_MAX_WIDTH);
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("bash", 24), "bash");
        let out = truncate("a-very-long-tab-title-that-keeps-going", 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
        assert_eq!(truncate("bash", 1), "…");
    }
}
