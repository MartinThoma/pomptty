//! The `Ctrl+R` fuzzy history-search overlay.
//!
//! A centered modal over the terminal: a search field on top, ranked results
//! below (see [`crate::history::log_store`]). `Enter` drops the selected command
//! on the prompt, `Ctrl+Enter` runs it, `Esc` closes.

use std::time::SystemTime;

use egui::{Align, Color32, FontId, Key, Modifiers, Rect, Sense, Stroke, pos2, vec2};

use crate::history::log_store::{Hit, LogStore, humanize_since};
use crate::ui::style::Surfaces;

/// How far `PageUp` / `PageDown` jump in the result list.
const PAGE_JUMP: usize = 8;
const ROW_TWO_LINE: f32 = 40.0;
const ROW_ONE_LINE: f32 = 26.0;

/// What the overlay decided this frame.
pub enum HistoryOutcome {
    /// Still open, nothing chosen.
    None,
    /// Put this command on the prompt without running it.
    Insert(String),
    /// Put this command on the prompt and press Enter.
    Run(String),
    /// Close without doing anything.
    Dismiss,
}

/// Overlay state. Lives on `PompttyApp` as an `Option`; `Some` means open.
pub struct HistoryOverlay {
    query: String,
    selected: usize,
    /// Restrict results to `cwd`.
    dir_only: bool,
    /// The active tab's working directory when the overlay opened (`None` if it
    /// couldn't be determined — the toggle is then hidden).
    cwd: Option<String>,
    /// Scroll the selected row into view on the next frame (set by key nav).
    scroll_pending: bool,
    first_frame: bool,
}

impl HistoryOverlay {
    pub fn new(cwd: Option<String>) -> Self {
        Self {
            query: String::new(),
            selected: 0,
            dir_only: false,
            cwd,
            scroll_pending: false,
            first_frame: true,
        }
    }

    fn cwd_filter(&self) -> Option<&str> {
        if self.dir_only {
            self.cwd.as_deref()
        } else {
            None
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        store: &LogStore,
        max_results: usize,
        s: Surfaces,
        dark: bool,
    ) -> HistoryOutcome {
        let hits = store.query(&self.query, self.cwd_filter(), max_results.max(1));
        if self.selected >= hits.len() {
            self.selected = hits.len().saturating_sub(1);
        }

        // Handle navigation keys before the text field is built so it doesn't
        // swallow the arrows / Enter.
        if let Some(outcome) = self.handle_keys(ctx, &hits) {
            return outcome;
        }

        let vis = ctx.animate_bool_with_time(egui::Id::new("pomptty_hist_anim"), true, 0.10);
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

        let mut clicked: Option<String> = None;

        let modal = egui::Modal::new(egui::Id::new("pomptty_history"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_opacity(vis);
                let w = (ui.ctx().content_rect().width() * 0.72).clamp(360.0, 640.0);
                ui.set_width(w);

                // Search field.
                ui.horizontal(|ui| {
                    search_icon(ui, s);
                    ui.add_space(4.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Search history…")
                            .desired_width(f32::INFINITY),
                    );
                    if self.first_frame || !resp.has_focus() {
                        resp.request_focus();
                    }
                    self.first_frame = false;
                });

                // "this directory only" toggle.
                if let Some(cwd) = self.cwd.clone() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let text = if self.dir_only {
                            format!("this directory only  ·  {}", collapse_home(&cwd))
                        } else {
                            "this directory only".to_owned()
                        };
                        ui.checkbox(
                            &mut self.dir_only,
                            egui::RichText::new(text).size(11.5).color(s.text_muted),
                        );
                        ui.label(egui::RichText::new("(Tab)").size(10.5).color(s.text_faint));
                    });
                }

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if hits.is_empty() {
                    ui.add_space(8.0);
                    let msg = if store.is_empty() {
                        "No history yet. Add  eval \"$(pomptty --print-integration <shell>)\"  \
                         to your shell's rc file."
                    } else {
                        "No matches."
                    };
                    ui.label(egui::RichText::new(msg).size(12.0).color(s.text_muted));
                    ui.add_space(8.0);
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (idx, hit) in hits.iter().enumerate() {
                                let selected = idx == self.selected;
                                let resp = draw_row(ui, s, hit, selected);
                                if resp.clicked() {
                                    clicked = Some(hit.command.clone());
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
                    egui::RichText::new(
                        "Enter insert  ·  Ctrl+Enter run  ·  Up/Down move  ·  Esc close",
                    )
                    .size(10.5)
                    .color(s.text_faint),
                );
            });

        if let Some(cmd) = clicked {
            return HistoryOutcome::Insert(cmd);
        }
        if modal.should_close() {
            return HistoryOutcome::Dismiss;
        }
        HistoryOutcome::None
    }

    /// Consume the navigation / accept keys. Returns `Some` when the user picked
    /// a command or dismissed.
    fn handle_keys(&mut self, ctx: &egui::Context, hits: &[Hit]) -> Option<HistoryOutcome> {
        let n = hits.len();
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
            if i.consume_key(Modifiers::NONE, Key::PageDown) {
                self.selected = self
                    .selected
                    .saturating_add(PAGE_JUMP)
                    .min(n.saturating_sub(1));
                self.scroll_pending = true;
            }
            if i.consume_key(Modifiers::NONE, Key::PageUp) {
                self.selected = self.selected.saturating_sub(PAGE_JUMP);
                self.scroll_pending = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Tab) && self.cwd.is_some() {
                self.dir_only = !self.dir_only;
            }
            if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                outcome = hits
                    .get(self.selected)
                    .map(|h| HistoryOutcome::Run(h.command.clone()));
            } else if i.consume_key(Modifiers::NONE, Key::Enter) {
                outcome = hits
                    .get(self.selected)
                    .map(|h| HistoryOutcome::Insert(h.command.clone()));
            }
        });
        outcome
    }
}

fn draw_row(ui: &mut egui::Ui, s: Surfaces, hit: &Hit, selected: bool) -> egui::Response {
    let two_line = hit.cwd.is_some();
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
    let cmd_y = rect.top() + if two_line { 6.0 } else { 5.0 };

    // Exit-status dot.
    p.circle_filled(
        pos2(rect.left() + 12.0, cmd_y + 6.5),
        3.0,
        match hit.exit_code {
            Some(0) => s.ok,
            Some(_) => s.err,
            None => s.text_faint,
        },
    );

    // Right-aligned meta: run count + relative time.
    let meta = meta_text(hit);
    let meta_galley = p.layout_no_wrap(meta, FontId::proportional(11.0), s.text_muted);
    let meta_x = rect.right() - 10.0 - meta_galley.size().x;
    p.galley(pos2(meta_x, cmd_y + 1.0), meta_galley, s.text_muted);

    // Command line (monospace), clipped to the space left of the meta.
    let text_left = rect.left() + 26.0;
    let clip = Rect::from_min_max(
        pos2(text_left, rect.top()),
        pos2(meta_x - 8.0, rect.bottom()),
    );
    let cmd_galley = p.layout_no_wrap(hit.command.clone(), FontId::monospace(13.0), s.text);
    p.with_clip_rect(clip)
        .galley(pos2(text_left, cmd_y), cmd_galley, s.text);

    if let Some(cwd) = &hit.cwd {
        let g = p.layout_no_wrap(collapse_home(cwd), FontId::proportional(10.5), s.text_faint);
        p.with_clip_rect(clip)
            .galley(pos2(text_left, rect.top() + 22.0), g, s.text_faint);
    }

    resp
}

fn meta_text(hit: &Hit) -> String {
    let age = humanize_since(hit.last_run, SystemTime::now());
    if hit.count > 1 {
        format!("×{}    {age}", hit.count)
    } else {
        age
    }
}

/// A small magnifier icon, painted so it needs no glyph font.
fn search_icon(ui: &mut egui::Ui, s: Surfaces) {
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let c = rect.center() - vec2(1.0, 1.0);
    let stroke = Stroke::new(1.6, s.text_muted);
    let p = ui.painter();
    p.circle_stroke(c, 4.2, stroke);
    let a = c + vec2(3.0, 3.0);
    p.line_segment([a, a + vec2(3.8, 3.8)], stroke);
}

/// Replace a leading `$HOME` with `~`.
fn collapse_home(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        if path == home {
            return "~".to_owned();
        }
        if let Some(rest) = path.strip_prefix(&format!("{home}/")) {
            return format!("~/{rest}");
        }
    }
    path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::log_store::Hit;
    use std::time::Duration;

    fn hit(command: &str, count: u32) -> Hit {
        Hit {
            command: command.to_owned(),
            cwd: None,
            exit_code: Some(0),
            last_run: SystemTime::now() - Duration::from_secs(120),
            count,
        }
    }

    #[test]
    fn meta_text_shows_count_only_when_repeated() {
        assert_eq!(meta_text(&hit("ls", 1)), "2m");
        assert_eq!(meta_text(&hit("ls", 4)), "×4    2m");
    }

    #[test]
    fn collapse_home_rewrites_the_prefix() {
        // SAFETY: single-threaded test process.
        unsafe { std::env::set_var("HOME", "/home/tester") };
        assert_eq!(collapse_home("/home/tester"), "~");
        assert_eq!(collapse_home("/home/tester/src/main.rs"), "~/src/main.rs");
        assert_eq!(collapse_home("/etc/hosts"), "/etc/hosts");
    }
}
