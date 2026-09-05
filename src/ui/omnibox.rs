//! The command palette (`Ctrl+Shift+P`): one input over four sources —
//! actions, tabs, history, and recent directories — grouped and ranked,
//! `Enter` to act. The browser-omnibox counterpart to
//! [`crate::ui::history_overlay`] (history alone) and
//! [`crate::ui::tab_search`] (tabs alone).

use egui::{Align, Color32, FontId, Key, Modifiers, Rect, Sense, Stroke, pos2, vec2};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::config::keybindings::Action;
use crate::history::log_store::{Hit, LogStore};
use crate::terminal::TabId;
use crate::ui::history_overlay::{meta_text, search_icon};
use crate::ui::style::Surfaces;
use crate::ui::tab_search::TabEntry;
use crate::ui::util::collapse_home;

const ROW_TWO_LINE: f32 = 40.0;
const ROW_ONE_LINE: f32 = 26.0;
/// Rows kept per section; sections that end up empty are skipped entirely.
const SECTION_LIMIT: usize = 6;

/// Actions worth reaching from the palette. Deliberately a curated subset of
/// [`Action`], not every variant — `GotoTab`/`Copy`/`Paste`/`Disabled` either
/// don't make sense here or are covered by the Tabs section.
const PALETTE_ACTIONS: &[(&str, Action)] = &[
    ("New Tab", Action::NewTab),
    ("Reopen Closed Tab", Action::ReopenTab),
    ("Close Tab", Action::CloseTab),
    ("Next Tab", Action::NextTab),
    ("Previous Tab", Action::PrevTab),
    ("Search Tabs", Action::TabSearch),
    ("Search History", Action::HistorySearch),
    ("Reload Config", Action::ReloadConfig),
    ("Clear Screen", Action::Clear),
    ("Reset Font Size", Action::FontReset),
    ("Scroll to Top", Action::ScrollToTop),
    ("Scroll to Bottom", Action::ScrollToBottom),
    ("View: Maximize Window", Action::WindowMaximize),
    ("View: Restore Window", Action::WindowRestore),
    ("View: Move to Left Half", Action::WindowLeftHalf),
    ("View: Move to Right Half", Action::WindowRightHalf),
];

/// What the palette decided this frame.
pub enum OmniboxOutcome {
    None,
    Dismiss,
    RunAction(Action),
    SelectTab(TabId),
    InsertCommand(String),
    RunCommand(String),
    ChangeDir(String),
}

/// One row, already resolved to its section. Built fresh from a query on every
/// frame — the underlying snapshots (`tabs`, `dirs`) are cheap and small.
enum OmniItem {
    Action {
        action: Action,
        label: &'static str,
    },
    Tab {
        id: TabId,
        title: String,
        cwd: Option<String>,
    },
    History(Hit),
    Dir(String),
}

impl OmniItem {
    fn section(&self) -> &'static str {
        match self {
            OmniItem::Action { .. } => "Actions",
            OmniItem::Tab { .. } => "Tabs",
            OmniItem::History(_) => "History",
            OmniItem::Dir(_) => "Directories",
        }
    }

    /// What picking this row means. `run` is the `Ctrl+Enter` variant —
    /// meaningful only for History rows (run instead of insert).
    fn act(&self, run: bool) -> OmniboxOutcome {
        match self {
            OmniItem::Action { action, .. } => OmniboxOutcome::RunAction(*action),
            OmniItem::Tab { id, .. } => OmniboxOutcome::SelectTab(*id),
            OmniItem::History(hit) if run => OmniboxOutcome::RunCommand(hit.command.clone()),
            OmniItem::History(hit) => OmniboxOutcome::InsertCommand(hit.command.clone()),
            OmniItem::Dir(dir) => OmniboxOutcome::ChangeDir(dir.clone()),
        }
    }
}

pub struct OmniboxOverlay {
    query: String,
    selected: usize,
    first_frame: bool,
    scroll_pending: bool,
    tabs: Vec<TabEntry>,
    active_tab: TabId,
    /// Recent distinct directories from the history log, most-recent first —
    /// the active tab's own directory already filtered out by the caller.
    dirs: Vec<String>,
}

impl OmniboxOverlay {
    pub fn new(tabs: Vec<TabEntry>, active_tab: TabId, dirs: Vec<String>) -> Self {
        Self {
            query: String::new(),
            selected: 0,
            first_frame: true,
            scroll_pending: false,
            tabs,
            active_tab,
            dirs,
        }
    }

    /// The four sections, ranked and concatenated, empty ones skipped.
    fn build_items(&self, history: &LogStore) -> Vec<OmniItem> {
        let query = self.query.trim();
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

        let mut items = Vec::new();
        for &(label, action) in rank(&pattern, &mut matcher, PALETTE_ACTIONS, |a| a.0.to_owned()) {
            items.push(OmniItem::Action { action, label });
        }
        for entry in rank(&pattern, &mut matcher, &self.tabs, |e| {
            format!("{} {}", e.title, e.cwd.as_deref().unwrap_or(""))
        }) {
            items.push(OmniItem::Tab {
                id: entry.id,
                title: entry.title.clone(),
                cwd: entry.cwd.clone(),
            });
        }
        for hit in history.query(query, None, SECTION_LIMIT) {
            items.push(OmniItem::History(hit));
        }
        for dir in rank(&pattern, &mut matcher, &self.dirs, String::clone) {
            items.push(OmniItem::Dir(dir.clone()));
        }
        items
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        history: &LogStore,
        s: Surfaces,
        dark: bool,
    ) -> OmniboxOutcome {
        let items = self.build_items(history);
        if self.selected >= items.len() {
            self.selected = items.len().saturating_sub(1);
        }

        // Handle navigation keys before the text field is built so it doesn't
        // swallow the arrows / Enter.
        if let Some(outcome) = self.handle_keys(ctx, &items) {
            return outcome;
        }

        let vis = ctx.animate_bool_with_time(egui::Id::new("pomptty_omnibox_anim"), true, 0.10);
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

        let mut clicked: Option<usize> = None;

        let modal = egui::Modal::new(egui::Id::new("pomptty_omnibox"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_opacity(vis);
                let w = (ui.ctx().content_rect().width() * 0.72).clamp(380.0, 640.0);
                ui.set_width(w);

                ui.horizontal(|ui| {
                    search_icon(ui, s);
                    ui.add_space(4.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Run a command, switch tab, cd…")
                            .desired_width(f32::INFINITY),
                    );
                    if self.first_frame || !resp.has_focus() {
                        resp.request_focus();
                    }
                    self.first_frame = false;
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if items.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("No matches.")
                            .size(12.0)
                            .color(s.text_muted),
                    );
                    ui.add_space(8.0);
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(380.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let mut section = None;
                            for (idx, item) in items.iter().enumerate() {
                                if section != Some(item.section()) {
                                    if section.is_some() {
                                        ui.add_space(6.0);
                                    }
                                    ui.label(
                                        egui::RichText::new(item.section())
                                            .size(10.5)
                                            .strong()
                                            .color(s.text_faint),
                                    );
                                    section = Some(item.section());
                                }
                                let selected = idx == self.selected;
                                let resp = draw_row(ui, s, item, self.active_tab, selected);
                                if resp.clicked() {
                                    clicked = Some(idx);
                                }
                                if selected && self.scroll_pending {
                                    ui.scroll_to_rect(resp.rect, Some(Align::Center));
                                }
                            }
                        });
                    self.scroll_pending = false;
                }

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Enter act    Ctrl+Enter run    Up/Down move    Esc close")
                        .size(10.5)
                        .color(s.text_faint),
                );
            });

        if let Some(idx) = clicked {
            return items
                .get(idx)
                .map_or(OmniboxOutcome::None, |it| it.act(false));
        }
        if modal.should_close() {
            return OmniboxOutcome::Dismiss;
        }
        OmniboxOutcome::None
    }

    fn handle_keys(&mut self, ctx: &egui::Context, items: &[OmniItem]) -> Option<OmniboxOutcome> {
        let n = items.len();
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
                self.scroll_pending = true;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp)
                || i.consume_key(Modifiers::COMMAND, Key::P)
            {
                self.selected = step(self.selected, -1);
                self.scroll_pending = true;
            }
            if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                outcome = items.get(self.selected).map(|it| it.act(true));
            } else if i.consume_key(Modifiers::NONE, Key::Enter) {
                outcome = items.get(self.selected).map(|it| it.act(false));
            }
        });
        outcome
    }
}

/// Score every item's `text(item)` against `pattern`, sort by relevance
/// (descending), and keep the top [`SECTION_LIMIT`]. An empty `pattern` scores
/// everything `0`, so a stable sort keeps `items`' own order — the "sensible
/// defaults before you type" behaviour, for free.
fn rank<'a, T>(
    pattern: &Pattern,
    matcher: &mut Matcher,
    items: &'a [T],
    text: impl Fn(&T) -> String,
) -> Vec<&'a T> {
    let mut scored: Vec<(&T, u32)> = items
        .iter()
        .filter_map(|it| {
            let hay = text(it);
            let mut buf = Vec::new();
            pattern
                .score(Utf32Str::new(&hay, &mut buf), matcher)
                .map(|score| (it, score))
        })
        .collect();
    scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    scored.truncate(SECTION_LIMIT);
    scored.into_iter().map(|(it, _)| it).collect()
}

fn draw_row(
    ui: &mut egui::Ui,
    s: Surfaces,
    item: &OmniItem,
    active_tab: TabId,
    selected: bool,
) -> egui::Response {
    let two_line = matches!(
        item,
        OmniItem::History(_) | OmniItem::Tab { cwd: Some(_), .. }
    );
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
    let text_y = rect.top() + if two_line { 6.0 } else { 5.0 };

    match item {
        OmniItem::Action { label, .. } => {
            let g = p.layout_no_wrap((*label).to_owned(), FontId::proportional(13.0), s.text);
            p.galley(pos2(rect.left() + 14.0, text_y), g, s.text);
        }
        OmniItem::Tab { id, title, cwd } => {
            if *id == active_tab {
                p.circle_filled(pos2(rect.left() + 12.0, text_y + 6.5), 3.0, s.accent);
            }
            let text_left = rect.left() + 26.0;
            let g = p.layout_no_wrap(title.clone(), FontId::proportional(13.0), s.text);
            p.galley(pos2(text_left, text_y), g, s.text);
            if let Some(cwd) = cwd {
                let cg =
                    p.layout_no_wrap(collapse_home(cwd), FontId::proportional(10.5), s.text_faint);
                p.galley(pos2(text_left, rect.top() + 22.0), cg, s.text_faint);
            }
        }
        OmniItem::History(hit) => {
            p.circle_filled(
                pos2(rect.left() + 12.0, text_y + 6.5),
                3.0,
                match hit.exit_code {
                    Some(0) => s.ok,
                    Some(_) => s.err,
                    None => s.text_faint,
                },
            );
            let text_left = rect.left() + 26.0;
            let meta_galley =
                p.layout_no_wrap(meta_text(hit), FontId::proportional(11.0), s.text_muted);
            let meta_x = rect.right() - 10.0 - meta_galley.size().x;
            p.galley(pos2(meta_x, text_y + 1.0), meta_galley, s.text_muted);

            let clip = Rect::from_min_max(
                pos2(text_left, rect.top()),
                pos2(meta_x - 8.0, rect.bottom()),
            );
            let cmd_galley = p.layout_no_wrap(hit.command.clone(), FontId::monospace(13.0), s.text);
            p.with_clip_rect(clip)
                .galley(pos2(text_left, text_y), cmd_galley, s.text);
            if let Some(cwd) = &hit.cwd {
                let cg =
                    p.layout_no_wrap(collapse_home(cwd), FontId::proportional(10.5), s.text_faint);
                p.with_clip_rect(clip)
                    .galley(pos2(text_left, rect.top() + 22.0), cg, s.text_faint);
            }
        }
        OmniItem::Dir(dir) => {
            let g = p.layout_no_wrap(collapse_home(dir), FontId::monospace(13.0), s.text);
            p.galley(pos2(rect.left() + 14.0, text_y), g, s.text);
        }
    }

    resp
}
