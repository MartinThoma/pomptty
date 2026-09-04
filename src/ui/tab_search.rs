//! The `Ctrl+Shift+A` fuzzy tab switcher: a small sibling of
//! [`crate::ui::history_overlay`] over the open tabs instead of shell history.

use egui::{Color32, FontId, Key, Modifiers, Sense, Stroke, pos2, vec2};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::terminal::TabId;
use crate::ui::style::Surfaces;
use crate::ui::util::collapse_home;

const ROW_TWO_LINE: f32 = 38.0;
const ROW_ONE_LINE: f32 = 24.0;

/// What the switcher decided this frame.
pub enum TabSearchOutcome {
    None,
    Select(TabId),
    Dismiss,
}

/// One open tab as the switcher needs to see it, snapshotted when it opens.
pub struct TabEntry {
    pub id: TabId,
    pub title: String,
    pub cwd: Option<String>,
}

pub struct TabSearchOverlay {
    entries: Vec<TabEntry>,
    active_id: TabId,
    query: String,
    selected: usize,
    first_frame: bool,
}

impl TabSearchOverlay {
    pub fn new(entries: Vec<TabEntry>, active_id: TabId) -> Self {
        let selected = entries.iter().position(|e| e.id == active_id).unwrap_or(0);
        Self {
            entries,
            active_id,
            query: String::new(),
            selected,
            first_frame: true,
        }
    }

    /// Ranked indices into `entries` for the current query: fuzzy relevance,
    /// falling back to the open order when the query is empty.
    fn filtered(&self) -> Vec<usize> {
        let q = self.query.trim();
        if q.is_empty() {
            return (0..self.entries.len()).collect();
        }
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = Pattern::parse(q, CaseMatching::Smart, Normalization::Smart);
        let mut scored: Vec<(usize, u32)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let hay = format!("{} {}", e.title, e.cwd.as_deref().unwrap_or(""));
                let mut buf = Vec::new();
                pattern
                    .score(Utf32Str::new(&hay, &mut buf), &mut matcher)
                    .map(|score| (i, score))
            })
            .collect();
        scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        scored.into_iter().map(|(i, _)| i).collect()
    }

    pub fn show(&mut self, ctx: &egui::Context, s: Surfaces, dark: bool) -> TabSearchOutcome {
        let matches = self.filtered();
        if self.selected >= matches.len() {
            self.selected = matches.len().saturating_sub(1);
        }

        if let Some(outcome) = self.handle_keys(ctx, &matches) {
            return outcome;
        }

        let vis = ctx.animate_bool_with_time(egui::Id::new("pomptty_tabsearch_anim"), true, 0.10);
        let shadow_alpha = if dark { 130 } else { 55 };
        let frame = egui::Frame::new()
            .fill(s.raised)
            .stroke(Stroke::new(1.0, s.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(14))
            .shadow(egui::Shadow {
                offset: [0, 14],
                blur: 40,
                spread: 0,
                color: Color32::from_black_alpha(shadow_alpha),
            });

        let mut clicked: Option<TabId> = None;

        let modal = egui::Modal::new(egui::Id::new("pomptty_tab_search"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_opacity(vis);
                let w = (ui.ctx().content_rect().width() * 0.6).clamp(320.0, 520.0);
                ui.set_width(w);

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Switch to tab…")
                        .desired_width(f32::INFINITY),
                );
                if self.first_frame || !resp.has_focus() {
                    resp.request_focus();
                }
                self.first_frame = false;

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if matches.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("No matching tabs.")
                            .size(12.0)
                            .color(s.text_muted),
                    );
                    ui.add_space(8.0);
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (row, &i) in matches.iter().enumerate() {
                                let entry = &self.entries[i];
                                let selected = row == self.selected;
                                let resp =
                                    draw_row(ui, s, entry, entry.id == self.active_id, selected);
                                if resp.clicked() {
                                    clicked = Some(entry.id);
                                }
                            }
                        });
                }

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Enter switch    Up/Down move    Esc close")
                        .size(10.5)
                        .color(s.text_faint),
                );
            });

        if let Some(id) = clicked {
            return TabSearchOutcome::Select(id);
        }
        if modal.should_close() {
            return TabSearchOutcome::Dismiss;
        }
        TabSearchOutcome::None
    }

    fn handle_keys(&mut self, ctx: &egui::Context, matches: &[usize]) -> Option<TabSearchOutcome> {
        let n = matches.len();
        let step = |cur: usize, delta: isize| -> usize {
            if n == 0 {
                0
            } else {
                (cur as isize + delta).rem_euclid(n as isize) as usize
            }
        };

        let mut outcome = None;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown)
                || i.consume_key(Modifiers::COMMAND, Key::N)
            {
                self.selected = step(self.selected, 1);
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp)
                || i.consume_key(Modifiers::COMMAND, Key::P)
            {
                self.selected = step(self.selected, -1);
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                outcome = matches
                    .get(self.selected)
                    .map(|&i| TabSearchOutcome::Select(self.entries[i].id));
            }
        });
        outcome
    }
}

fn draw_row(
    ui: &mut egui::Ui,
    s: Surfaces,
    entry: &TabEntry,
    is_active: bool,
    selected: bool,
) -> egui::Response {
    let two_line = entry.cwd.is_some();
    let h = if two_line { ROW_TWO_LINE } else { ROW_ONE_LINE };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());

    let bg = if selected {
        s.pressed
    } else if resp.hovered() {
        s.hover
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 6.0, bg);
    }

    let p = ui.painter();
    let text_y = rect.top() + if two_line { 5.0 } else { 3.0 };

    // A dot on the currently-active tab.
    if is_active {
        p.circle_filled(pos2(rect.left() + 12.0, text_y + 6.5), 3.0, s.accent);
    }

    let text_left = rect.left() + 26.0;
    let title_galley = p.layout_no_wrap(entry.title.clone(), FontId::proportional(13.0), s.text);
    p.galley(pos2(text_left, text_y), title_galley, s.text);

    if let Some(cwd) = &entry.cwd {
        let cwd_galley =
            p.layout_no_wrap(collapse_home(cwd), FontId::proportional(10.5), s.text_faint);
        p.galley(pos2(text_left, rect.top() + 20.0), cwd_galley, s.text_faint);
    }

    resp
}
