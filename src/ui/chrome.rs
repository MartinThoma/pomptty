//! The top bar: a minimal, Chrome-flavored tab strip.

use egui::{Color32, CornerRadius, Id, Rect, Sense, Stroke, Vec2, pos2, vec2};

use crate::terminal::TabId;
use crate::ui::style::{Surfaces, mix};

/// What the user asked the chrome to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeAction {
    None,
    NewTab,
    SelectTab(usize),
    CloseTab(usize),
    /// The `Ctrl+R` search affordance was clicked.
    OpenSearch,
    /// Custom-decoration window controls (only when `window_controls`).
    Minimize,
    ToggleMaximize,
    CloseWindow,
    /// The title-bar drag region was grabbed — start moving the window.
    BeginWindowDrag,
}

/// One tab, as the strip needs to see it.
#[derive(Clone, Copy)]
pub struct TabView<'a> {
    pub id: TabId,
    pub title: &'a str,
}

pub struct TabStrip<'a> {
    pub tabs: &'a [TabView<'a>],
    pub active: usize,
    pub surfaces: Surfaces,
    /// Draw minimize / maximize / close and make the strip a window-drag region
    /// (custom decorations).
    pub window_controls: bool,
}

/// Space above the tabs so they "float" in the strip, Chrome-style.
const STRIP_PAD_TOP: i8 = 7;
const TAB_HEIGHT: f32 = 29.0;
const TAB_MIN_WIDTH: f32 = 46.0;
const TAB_MAX_WIDTH: f32 = 240.0;
const TAB_GAP: f32 = 4.0;
const TAB_PAD: f32 = 12.0;
const CLOSE_SIZE: f32 = 17.0;
const BTN_SIZE: f32 = 24.0;
const TAB_ROUNDING: u8 = 8;
/// Tab-open animation length, seconds.
const ENTER_SECS: f32 = 0.16;

impl TabStrip<'_> {
    /// Returns the chrome action plus `true` if an animation is still running
    /// (so the caller keeps repainting).
    pub fn show(self, ui: &mut egui::Ui) -> (ChromeAction, bool) {
        let mut action = ChromeAction::None;
        let s = self.surfaces;
        let now = ui.ctx().input(|i| i.time);
        let mut animating = false;

        let frame = egui::Frame::new()
            .fill(s.raised)
            .inner_margin(egui::Margin {
                left: 6,
                right: 6,
                top: STRIP_PAD_TOP,
                bottom: 0,
            });

        egui::Panel::top("pomptty_tab_strip")
            .frame(frame)
            .exact_size(STRIP_PAD_TOP as f32 + TAB_HEIGHT)
            .resizable(false)
            .show_separator_line(false)
            .show(ui, |ui| {
                // Drag region first (registered "below"), so tabs and buttons
                // drawn on top win their own clicks.
                if self.window_controls {
                    let drag = ui.interact(
                        ui.max_rect(),
                        Id::new("pomptty_titlebar_drag"),
                        Sense::click_and_drag(),
                    );
                    if drag.drag_started() {
                        action = ChromeAction::BeginWindowDrag;
                    }
                    if drag.double_clicked() {
                        action = ChromeAction::ToggleMaximize;
                    }
                }
                let maximized = ui.input(|i| i.viewport().maximized).unwrap_or(false);
                let mut active_rect = None;
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = vec2(TAB_GAP, 0.0);

                    let ctrl_w = if self.window_controls {
                        BTN_SIZE * 3.0 + 8.0
                    } else {
                        0.0
                    };
                    let reserve = BTN_SIZE + ctrl_w + 14.0;
                    egui::ScrollArea::horizontal()
                        .max_width((ui.available_width() - reserve).max(0.0))
                        .show(ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                ui.spacing_mut().item_spacing = vec2(TAB_GAP, 0.0);
                                let w = tab_width(ui.available_width(), self.tabs.len());
                                for (i, tab) in self.tabs.iter().enumerate() {
                                    let enter = enter_t(ui, tab.id, now);
                                    animating |= enter < 1.0;
                                    let r = draw_tab(
                                        ui,
                                        &s,
                                        tab.title,
                                        i == self.active,
                                        w,
                                        enter,
                                        Id::new(("pomptty_tab", tab.id)),
                                        &mut action,
                                        i,
                                    );
                                    if i == self.active {
                                        active_rect = Some(r);
                                    }
                                }
                                if icon_button(ui, &s, Glyph::Plus)
                                    .on_hover_text("New tab  ·  Ctrl+Shift+T")
                                    .clicked()
                                {
                                    action = ChromeAction::NewTab;
                                }
                            });
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.window_controls {
                            if icon_button(ui, &s, Glyph::Close)
                                .on_hover_text("Close window")
                                .clicked()
                            {
                                action = ChromeAction::CloseWindow;
                            }
                            let max_glyph = if maximized {
                                Glyph::Restore
                            } else {
                                Glyph::Maximize
                            };
                            if icon_button(ui, &s, max_glyph)
                                .on_hover_text(if maximized { "Restore" } else { "Maximize" })
                                .clicked()
                            {
                                action = ChromeAction::ToggleMaximize;
                            }
                            if icon_button(ui, &s, Glyph::Minimize)
                                .on_hover_text("Minimize")
                                .clicked()
                            {
                                action = ChromeAction::Minimize;
                            }
                            ui.add_space(4.0);
                        }
                        if icon_button(ui, &s, Glyph::Search)
                            .on_hover_text("Search history  ·  Ctrl+R")
                            .clicked()
                        {
                            action = ChromeAction::OpenSearch;
                        }
                    });
                });

                // Baseline + a faint drop shadow so the strip reads as raised.
                let rect = ui.max_rect();
                let base_y = rect.bottom();
                for (i, a) in [10u8, 6, 3].into_iter().enumerate() {
                    ui.painter().hline(
                        rect.left()..=rect.right(),
                        base_y + i as f32,
                        Stroke::new(1.0, Color32::from_black_alpha(a)),
                    );
                }
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    base_y - 0.5,
                    Stroke::new(1.0, s.border),
                );

                // The active-tab accent, eased toward the active tab's slot.
                if let Some(r) = active_rect {
                    let ax = ui.ctx().animate_value_with_time(
                        Id::new("pomptty_accent_x"),
                        r.left(),
                        0.12,
                    );
                    let aw = ui.ctx().animate_value_with_time(
                        Id::new("pomptty_accent_w"),
                        r.width(),
                        0.12,
                    );
                    animating |= (ax - r.left()).abs() > 0.5 || (aw - r.width()).abs() > 0.5;
                    ui.painter().hline(
                        ax + 1.0..=ax + aw - 1.0,
                        r.top() + 1.0,
                        Stroke::new(2.0, s.accent),
                    );
                    // Punch the active tab flush through the baseline.
                    ui.painter()
                        .hline(r.left()..=r.right(), base_y - 0.5, Stroke::new(1.5, s.bg));
                }
            });

        (action, animating)
    }
}

/// Seconds since this tab id was first drawn, as an eased 0→1 ramp. State lives
/// in egui memory so the strip stays stateless.
fn enter_t(ui: &egui::Ui, id: TabId, now: f64) -> f32 {
    let key = Id::new(("pomptty_tab_born", id));
    let born = ui
        .ctx()
        .data_mut(|d| *d.get_temp_mut_or_insert_with(key, || now));
    let t = ((now - born) as f32 / ENTER_SECS).clamp(0.0, 1.0);
    egui::emath::easing::cubic_out(t)
}

#[allow(clippy::too_many_arguments)]
fn draw_tab(
    ui: &mut egui::Ui,
    s: &Surfaces,
    title: &str,
    selected: bool,
    target_width: f32,
    enter: f32,
    id: Id,
    action: &mut ChromeAction,
    index: usize,
) -> Rect {
    let width = target_width * enter.max(0.02);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::click());
    let hovered = resp.hovered();
    let show_close = selected || hovered;

    let fill = if selected {
        s.bg
    } else if hovered {
        s.hover
    } else {
        Color32::TRANSPARENT
    };
    let rounding = CornerRadius {
        nw: TAB_ROUNDING,
        ne: TAB_ROUNDING,
        sw: 0,
        se: 0,
    };
    ui.painter().rect_filled(rect, rounding, fill);
    if !selected && !hovered {
        // Faint divider between resting tabs.
        ui.painter().vline(
            (rect.right() + TAB_GAP / 2.0).round(),
            (rect.top() + 9.0)..=(rect.bottom() - 9.0),
            Stroke::new(1.0, s.text_faint.gamma_multiply(0.35)),
        );
    }

    let close_rect = Rect::from_center_size(
        pos2(rect.right() - 6.0 - CLOSE_SIZE / 2.0, rect.center().y),
        Vec2::splat(CLOSE_SIZE),
    );
    let close = ui.interact(close_rect, id.with("close"), Sense::click());
    let text_color = if selected { s.text } else { s.text_muted };
    if show_close && width > CLOSE_SIZE + TAB_PAD {
        paint_close(ui, close_rect, &close, s);
    }

    let clip_right = if show_close && width > CLOSE_SIZE + TAB_PAD {
        close_rect.left() - 4.0
    } else {
        rect.right() - 8.0
    };
    let text_clip = Rect::from_min_max(
        pos2(rect.left() + TAB_PAD, rect.top()),
        pos2(clip_right, rect.bottom()),
    );
    if text_clip.width() > 2.0 {
        let budget = (text_clip.width() / 6.9).floor().max(1.0) as usize;
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

    let on_close = ui
        .input(|i| i.pointer.interact_pos())
        .is_some_and(|p| close_rect.contains(p));
    if close.clicked()
        || (resp.clicked() && on_close)
        || resp.clicked_by(egui::PointerButton::Middle)
    {
        *action = ChromeAction::CloseTab(index);
    } else if resp.clicked() {
        *action = ChromeAction::SelectTab(index);
    }

    rect
}

/// Width for each tab: share the available space, Chrome-style, clamped.
fn tab_width(available: f32, count: usize) -> f32 {
    if count == 0 {
        return TAB_MAX_WIDTH;
    }
    let each = available / count as f32 - TAB_GAP;
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
fn paint_close(ui: &egui::Ui, rect: Rect, resp: &egui::Response, s: &Surfaces) {
    let p = ui.painter();
    if resp.hovered() {
        p.circle_filled(rect.center(), CLOSE_SIZE / 2.0, s.err.gamma_multiply(0.85));
    }
    let color = if resp.hovered() {
        Color32::WHITE
    } else {
        s.text_faint
    };
    let r = 3.2;
    let c = rect.center();
    let stroke = Stroke::new(1.5, color);
    p.line_segment([c + vec2(-r, -r), c + vec2(r, r)], stroke);
    p.line_segment([c + vec2(-r, r), c + vec2(r, -r)], stroke);
}

#[derive(Clone, Copy)]
enum Glyph {
    Plus,
    Search,
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn icon_button(ui: &mut egui::Ui, s: &Surfaces, glyph: Glyph) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BTN_SIZE), Sense::click());
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    let danger = matches!(glyph, Glyph::Close);
    if t > 0.0 {
        let fill = if danger {
            mix(Color32::TRANSPARENT, s.err, t * 0.9)
        } else {
            mix(Color32::TRANSPARENT, s.hover, t)
        };
        ui.painter().rect_filled(rect, 6, fill);
    }
    let color = if danger && t > 0.0 {
        Color32::WHITE
    } else {
        mix(s.text_muted, s.text, t)
    };
    let stroke = Stroke::new(1.6, color);
    let c = rect.center();
    let p = ui.painter();
    match glyph {
        Glyph::Plus => {
            let r = 4.5;
            p.line_segment([c + vec2(-r, 0.0), c + vec2(r, 0.0)], stroke);
            p.line_segment([c + vec2(0.0, -r), c + vec2(0.0, r)], stroke);
        }
        Glyph::Search => {
            let o = c - vec2(1.2, 1.2);
            p.circle_stroke(o, 4.2, stroke);
            let a = o + vec2(3.0, 3.0);
            p.line_segment([a, a + vec2(3.6, 3.6)], stroke);
        }
        Glyph::Minimize => {
            p.line_segment([c + vec2(-4.5, 3.0), c + vec2(4.5, 3.0)], stroke);
        }
        Glyph::Maximize => {
            let r = Rect::from_center_size(c, Vec2::splat(9.0));
            p.rect_stroke(r, 1, stroke, egui::StrokeKind::Middle);
        }
        Glyph::Restore => {
            let back = Rect::from_min_size(c + vec2(-2.5, -4.5), Vec2::splat(7.0));
            let front = Rect::from_min_size(c + vec2(-4.5, -2.5), Vec2::splat(7.0));
            p.rect_stroke(back, 1, stroke, egui::StrokeKind::Middle);
            p.rect_filled(front, 1, s.raised);
            p.rect_stroke(front, 1, stroke, egui::StrokeKind::Middle);
        }
        Glyph::Close => {
            let r = 4.0;
            p.line_segment([c + vec2(-r, -r), c + vec2(r, r)], stroke);
            p.line_segment([c + vec2(-r, r), c + vec2(r, -r)], stroke);
        }
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::{TAB_MAX_WIDTH, TAB_MIN_WIDTH, tab_width, truncate};

    #[test]
    fn tab_width_is_clamped() {
        assert_eq!(tab_width(2000.0, 1), TAB_MAX_WIDTH);
        assert_eq!(tab_width(100.0, 20), TAB_MIN_WIDTH);
        let mid = tab_width(500.0, 3);
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
