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
    /// A tab was dragged from index `from` to index `to` (drag-to-reorder).
    MoveTab {
        from: usize,
        to: usize,
    },
    /// Context menu: rename tab `i`.
    RenameTab(usize),
    /// Context menu: open a copy of tab `i` (same directory) right after it.
    DuplicateTab(usize),
    /// Context menu: close every tab except `i`.
    CloseOtherTabs(usize),
    /// Context menu: reopen the closed tab at position `k` in the recent list.
    ReopenClosedTab(usize),
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
    /// Titles of recently-closed tabs, newest first, for the context menu's
    /// "Reopen closed" submenu.
    pub closed: &'a [&'a str],
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
/// How fast a tab slides to a new slot (reorder, or a neighbour closing).
const POS_SECS: f32 = 0.12;

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
                                let (act, anim, arect) =
                                    tab_row(ui, &s, self.tabs, self.active, self.closed, now);
                                animating |= anim;
                                if act != ChromeAction::None {
                                    action = act;
                                }
                                if arect.is_some() {
                                    active_rect = arect;
                                }
                                if load_drag(ui).is_none()
                                    && icon_button(ui, &s, Glyph::Plus)
                                        .on_hover_text("New tab")
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

/// A tab currently being dragged, kept in egui memory so the strip stays
/// stateless between frames. `grab_dx` is the pointer's offset from the tab's
/// left edge at the moment it was grabbed.
#[derive(Clone, Copy)]
struct DragState {
    tab_id: TabId,
    grab_dx: f32,
}

const DRAG_KEY: &str = "pomptty_tab_drag";

fn load_drag(ui: &egui::Ui) -> Option<DragState> {
    ui.ctx()
        .data(|d| d.get_temp::<DragState>(Id::new(DRAG_KEY)))
}
fn store_drag(ui: &egui::Ui, ds: DragState) {
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(DRAG_KEY), ds));
}
fn clear_drag(ui: &egui::Ui) {
    ui.ctx()
        .data_mut(|d| d.remove::<DragState>(Id::new(DRAG_KEY)));
}

/// The close-`×` hit area inside a tab.
fn close_rect_of(tab: Rect) -> Rect {
    Rect::from_center_size(
        pos2(tab.right() - 6.0 - CLOSE_SIZE / 2.0, tab.center().y),
        Vec2::splat(CLOSE_SIZE),
    )
}

/// Which slot a tab whose centre sits at `centre_x` belongs in, given the row
/// starts at `left` and each slot (tab + gap) is `stride` wide. Clamped to
/// `0..n`.
fn drop_index(centre_x: f32, left: f32, stride: f32, n: usize) -> usize {
    if n == 0 || stride <= 0.0 {
        return 0;
    }
    let slot = ((centre_x - left) / stride).floor();
    (slot.max(0.0) as usize).min(n - 1)
}

/// Lay out and paint the whole tab row, handling drag-to-reorder. Returns any
/// chrome action, whether an animation is still running, and the active tab's
/// on-screen rect (for the accent underline).
fn tab_row(
    ui: &mut egui::Ui,
    s: &Surfaces,
    tabs: &[TabView<'_>],
    active: usize,
    closed: &[&str],
    now: f64,
) -> (ChromeAction, bool, Option<Rect>) {
    let n = tabs.len();
    if n == 0 {
        return (ChromeAction::None, false, None);
    }
    let mut action = ChromeAction::None;
    let mut animating = false;

    let tab_w = tab_width(ui.available_width(), n);
    let stride = tab_w + TAB_GAP;
    let enters: Vec<f32> = tabs.iter().map(|t| enter_t(ui, t.id, now)).collect();
    let widths: Vec<f32> = enters.iter().map(|e| tab_w * e.max(0.02)).collect();
    animating |= enters.iter().any(|e| *e < 1.0);

    // Resting slot x for each tab (running sum, so a still-opening tab pushes its
    // neighbours over rather than overlapping them).
    let mut total = 0.0_f32;
    let rel_x: Vec<f32> = widths
        .iter()
        .map(|w| {
            let x = total;
            total += w + TAB_GAP;
            x
        })
        .collect();
    total = (total - TAB_GAP).max(0.0);

    let (row_rect, _) = ui.allocate_exact_size(vec2(total, TAB_HEIGHT), Sense::hover());
    let left = row_rect.left();
    let top = row_rect.top();
    let xs: Vec<f32> = rel_x.iter().map(|x| left + x).collect();

    let drag = load_drag(ui);
    let pointer = ui.input(|i| i.pointer.interact_pos());

    let mut rects: Vec<Rect> = Vec::with_capacity(n);
    let mut dragged: Option<usize> = None;
    let mut active_rect = None;

    for (i, tab) in tabs.iter().enumerate() {
        let is_dragged = drag.map(|d| d.tab_id) == Some(tab.id);
        let x = match (is_dragged, pointer, drag) {
            (true, Some(p), Some(d)) => {
                (p.x - d.grab_dx).clamp(left, (left + total - widths[i]).max(left))
            }
            (false, _, _) => {
                let ax = ui.ctx().animate_value_with_time(
                    Id::new(("pomptty_tab_x", tab.id)),
                    xs[i],
                    POS_SECS,
                );
                animating |= (ax - xs[i]).abs() > 0.5;
                ax
            }
            _ => xs[i],
        };
        let r = Rect::from_min_size(
            pos2(x, top - if is_dragged { 2.0 } else { 0.0 }),
            vec2(widths[i], TAB_HEIGHT),
        );
        rects.push(r);

        let resp = ui.interact(r, Id::new(("pomptty_tab", tab.id)), Sense::click_and_drag());
        if resp.drag_started() {
            if let Some(p) = pointer {
                store_drag(
                    ui,
                    DragState {
                        tab_id: tab.id,
                        grab_dx: p.x - x,
                    },
                );
            }
            action = ChromeAction::SelectTab(i);
        }
        if resp.drag_stopped() {
            clear_drag(ui);
        }

        if is_dragged {
            // Pin the eased position to the cursor so releasing settles smoothly
            // into the tab's new slot.
            ui.ctx()
                .animate_value_with_time(Id::new(("pomptty_tab_x", tab.id)), x, 0.0);
            let to = drop_index(r.center().x, left, stride, n);
            if to != i {
                action = ChromeAction::MoveTab { from: i, to };
            }
            animating = true;
            dragged = Some(i);
            if i == active {
                active_rect = Some(r);
            }
            continue;
        }

        resp.clone().on_hover_text(tab.title);
        paint_tab(
            ui,
            s,
            tab.title,
            i == active,
            enters[i],
            r,
            resp.hovered(),
            false,
        );

        let on_close = pointer.is_some_and(|p| close_rect_of(r).contains(p));
        if (resp.clicked() && on_close) || resp.clicked_by(egui::PointerButton::Middle) {
            action = ChromeAction::CloseTab(i);
        } else if resp.clicked() {
            action = ChromeAction::SelectTab(i);
        }

        resp.context_menu(|ui| {
            if ui.button("Rename…").clicked() {
                action = ChromeAction::RenameTab(i);
                ui.close();
            }
            if ui.button("Duplicate").clicked() {
                action = ChromeAction::DuplicateTab(i);
                ui.close();
            }
            ui.separator();
            if ui.button("Close").clicked() {
                action = ChromeAction::CloseTab(i);
                ui.close();
            }
            if ui
                .add_enabled(n > 1, egui::Button::new("Close other tabs"))
                .clicked()
            {
                action = ChromeAction::CloseOtherTabs(i);
                ui.close();
            }
            if !closed.is_empty() {
                ui.separator();
                ui.menu_button("Reopen closed", |ui| {
                    for (k, title) in closed.iter().enumerate() {
                        if ui.button(*title).clicked() {
                            action = ChromeAction::ReopenClosedTab(k);
                            ui.close();
                        }
                    }
                });
            }
        });

        if i == active {
            active_rect = Some(r);
        }
    }

    match dragged {
        Some(i) => paint_tab(
            ui,
            s,
            tabs[i].title,
            i == active,
            enters[i],
            rects[i],
            false,
            true,
        ),
        // A drag whose tab vanished (e.g. its shell exited): drop the stale state.
        None if drag.is_some() => clear_drag(ui),
        None => {}
    }

    (action, animating, active_rect)
}

/// Paint one tab at `rect`. `dragging` gives it a lifted look; `enter` fades and
/// the caller has already narrowed `rect` for the open animation.
#[allow(clippy::too_many_arguments)]
fn paint_tab(
    ui: &egui::Ui,
    s: &Surfaces,
    title: &str,
    selected: bool,
    enter: f32,
    rect: Rect,
    hovered: bool,
    dragging: bool,
) {
    let a = enter.clamp(0.0, 1.0);
    let rounding = CornerRadius {
        nw: TAB_ROUNDING,
        ne: TAB_ROUNDING,
        sw: 0,
        se: 0,
    };

    if dragging {
        for (dy, alpha) in [(3.0_f32, 55_u8), (7.0, 24), (12.0, 9)] {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min + vec2(-1.0, dy), rect.size() + vec2(2.0, 0.0)),
                rounding,
                Color32::from_black_alpha(alpha),
            );
        }
    }

    let fill = if dragging || selected {
        s.bg
    } else if hovered {
        s.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, rounding, fill.gamma_multiply(a));

    if !dragging && !selected && !hovered {
        // Faint divider between resting tabs.
        ui.painter().vline(
            (rect.right() + TAB_GAP / 2.0).round(),
            (rect.top() + 9.0)..=(rect.bottom() - 9.0),
            Stroke::new(1.0, s.text_faint.gamma_multiply(0.35 * a)),
        );
    }

    let show_close = (selected || hovered) && !dragging && rect.width() > CLOSE_SIZE + TAB_PAD;
    let close_rect = close_rect_of(rect);
    if show_close {
        paint_close(ui, close_rect, ui.rect_contains_pointer(close_rect), s);
    }

    let clip_right = if show_close {
        close_rect.left() - 4.0
    } else {
        rect.right() - 8.0
    };
    let text_color = if selected || dragging {
        s.text
    } else {
        s.text_muted
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
            text_color.gamma_multiply(a),
        );
        let pos = pos2(text_clip.left(), rect.center().y - galley.size().y / 2.0);
        ui.painter()
            .with_clip_rect(text_clip)
            .galley(pos, galley, text_color.gamma_multiply(a));
    }
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
fn paint_close(ui: &egui::Ui, rect: Rect, hovered: bool, s: &Surfaces) {
    let p = ui.painter();
    if hovered {
        p.circle_filled(rect.center(), CLOSE_SIZE / 2.0, s.err.gamma_multiply(0.85));
    }
    let color = if hovered {
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
    use super::{TAB_MAX_WIDTH, TAB_MIN_WIDTH, drop_index, tab_width, truncate};

    #[test]
    fn drop_index_maps_a_centre_to_its_slot() {
        // Row at x=100, 5 tabs, slot stride 80 (tab 76 + gap 4).
        let (left, stride, n) = (100.0, 80.0, 5);
        // A resting tab's centre is left + i*stride + ~half a tab → its own slot.
        assert_eq!(drop_index(100.0 + 38.0, left, stride, n), 0);
        assert_eq!(drop_index(100.0 + 2.0 * 80.0 + 38.0, left, stride, n), 2);
        // Dragged past the next boundary.
        assert_eq!(drop_index(100.0 + 80.0 + 45.0, left, stride, n), 1);
        assert_eq!(drop_index(100.0 + 2.0 * 80.0 - 5.0, left, stride, n), 1);
        // Clamped at both ends.
        assert_eq!(drop_index(-500.0, left, stride, n), 0);
        assert_eq!(drop_index(9_999.0, left, stride, n), n - 1);
    }

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
