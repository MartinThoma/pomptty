//! The eframe application: owns the tabs, config, and the frame around them.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::Result;
use egui_term::{FontSettings, PtyEvent, TerminalFont, TerminalTheme, TerminalView};
use notify::{RecursiveMode, Watcher};

use crate::config::Config;
use crate::config::keybindings::{Action, Chord};
use crate::history::log_store::LogStore;
use crate::session::{Session, restore_active};
use crate::terminal::{TabColor, TabId, TerminalTab};
use crate::ui::chrome::{ChromeAction, TabStrip, TabView};
use crate::ui::history_overlay::{HistoryOutcome, HistoryOverlay};
use crate::ui::omnibox::{OmniboxOutcome, OmniboxOverlay};
use crate::ui::style::Surfaces;
use crate::ui::tab_search::{TabEntry, TabSearchOutcome, TabSearchOverlay};

/// How many lines a "page" scroll moves. A rough constant is fine — the backend
/// clamps at the ends of the scrollback.
const PAGE_LINES: i32 = 24;
const SCROLL_TO_EDGE: i32 = 1_000_000;
const TOAST_TTL: Duration = Duration::from_secs(5);
/// Uniform breathing room between the window edge and the terminal grid.
const TERMINAL_MARGIN: i8 = 8;

/// A transient status message shown at the bottom of the window.
struct Toast {
    msg: String,
    born: Instant,
    /// Set once the TTL elapses; the toast then fades out before it is dropped.
    dismissing: bool,
}

/// A user-closed tab, kept around so `reopen-tab` can bring it back. Only
/// user-initiated closes are recorded — a shell that exited on its own isn't
/// "reopenable".
struct ClosedTab {
    /// The tab's title at close time: the rename override if it had one,
    /// otherwise the shell-set title.
    title: String,
    cwd: Option<String>,
    color: Option<TabColor>,
    /// The slot it sat in, so reopening restores its position.
    index: usize,
}
/// How many closed tabs to remember.
const CLOSED_TABS_CAP: usize = 16;

/// Font-zoom limits, in points.
const FONT_MIN: f32 = 6.0;
const FONT_MAX: f32 = 72.0;
/// How long the font size must sit unchanged before it is written back to the
/// config file, so holding the zoom key doesn't rewrite it on every repeat.
const FONT_PERSIST_DELAY: Duration = Duration::from_millis(500);

/// How often to check the open tabs (and their directories) against the last
/// saved session, writing only when something actually changed. A `cd` inside
/// a shell isn't an event pomptty sees, so this poll — not a save-on-change
/// hook — is what keeps the saved session's directories fresh, and what makes
/// "restore after a crash" lose only a little rather than nothing.
const SESSION_SAVE_INTERVAL: Duration = Duration::from_secs(10);

pub struct PompttyApp {
    config: Config,
    config_path: PathBuf,
    theme: TerminalTheme,
    /// Whether pomptty draws its own window frame (from `window.decorations`;
    /// fixed at startup — the viewport flag can't change at runtime).
    custom_chrome: bool,
    /// For custom chrome: the configured size to re-assert on the first frame
    /// (some X11 WMs ignore the initial size of an undecorated window). `None`
    /// once done.
    initial_size: Option<egui::Vec2>,
    /// The live terminal font size in points (changed by the zoom keys).
    font_size: f32,
    /// The size `font-reset` returns to: the last value seen in the config file
    /// from an outside edit. Zoom write-backs do not move it.
    configured_font_size: f32,
    /// Set when the zoom keys change `font_size`; cleared once the new value has
    /// been persisted to the config file (see [`FONT_PERSIST_DELAY`]).
    font_dirty: bool,
    font_touched_at: Instant,

    tabs: Vec<TerminalTab>,
    active: usize,
    /// Next id to hand out to a new tab.
    next_tab_id: TabId,

    /// Cloned into each new tab so its PTY can post events back.
    pty_events_tx: Sender<(TabId, PtyEvent)>,
    pty_events_rx: Receiver<(TabId, PtyEvent)>,

    config_reload_rx: Receiver<()>,
    _config_watcher: Option<notify::RecommendedWatcher>,

    bindings: Vec<(Chord, Action)>,
    /// A tab whose close is waiting on the "a process is still running"
    /// confirmation, identified by id so a shifting `Vec` can't misfire it.
    pending_close: Option<TabId>,
    /// Command history, read from the shell-hook logs. Refreshed when the
    /// `Ctrl+R` overlay opens.
    history: LogStore,
    /// The `Ctrl+R` search overlay; `Some` while it is open.
    history_overlay: Option<HistoryOverlay>,
    /// User-closed tabs, most recent last. `reopen-tab` pops from here.
    closed_tabs: Vec<ClosedTab>,
    /// The `Ctrl+Shift+A` tab switcher; `Some` while it is open.
    tab_search: Option<TabSearchOverlay>,
    /// The `Ctrl+Shift+P` command palette; `Some` while it is open.
    omnibox: Option<OmniboxOverlay>,
    /// The tab a rename dialog is open for, and its live edit buffer.
    rename_target: Option<TabId>,
    rename_buf: String,
    /// The last time the open-tabs session was checked against disk.
    session_last_saved: Instant,
    /// What was last written (or loaded), to skip a no-op save.
    session_last_written: Option<Session>,
    toast: Option<Toast>,
    /// When the terminal last rang the bell — drives a brief screen flash.
    bell_at: Option<Instant>,
    /// The window title we last pushed, to avoid redundant viewport commands.
    title_shown: String,
}

impl PompttyApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        config_path: PathBuf,
        config_error: Option<String>,
    ) -> Result<Self> {
        let ctx = &cc.egui_ctx;
        crate::fonts::apply(ctx, &config);
        crate::ui::style::apply(ctx, &config.theme);

        let (pty_events_tx, pty_events_rx) = channel();

        // Restore the last session's tabs, if enabled and there is one.
        let session = config.session.restore.then(Session::load).flatten();
        let mut tabs = Vec::new();
        let mut next_tab_id: TabId = 0;
        for saved in session.iter().flat_map(|s| &s.tabs) {
            match TerminalTab::new(
                next_tab_id,
                ctx.clone(),
                pty_events_tx.clone(),
                config.shell.clone(),
                config.shell_args.clone(),
                saved.cwd.clone().map(PathBuf::from),
            ) {
                Ok(mut tab) => {
                    tab.manual_title = saved.title.clone();
                    tab.color = saved.color;
                    tabs.push(tab);
                    next_tab_id += 1;
                }
                Err(e) => log::warn!("could not restore a tab: {e:#}"),
            }
        }
        let active = session
            .as_ref()
            .map(|s| restore_active(s.active, tabs.len()))
            .unwrap_or(0);
        if tabs.is_empty() {
            // Nothing to restore (or restore is off, or every restored tab
            // failed to spawn): the one default tab, in pomptty's own cwd.
            let first = TerminalTab::new(
                next_tab_id,
                ctx.clone(),
                pty_events_tx.clone(),
                config.shell.clone(),
                config.shell_args.clone(),
                None,
            )?;
            next_tab_id += 1;
            tabs.push(first);
        }

        let (config_reload_rx, watcher) = spawn_config_watcher(&config_path, ctx.clone());

        let custom_chrome = config.window.decorations == crate::config::Decoration::Custom;
        let mut app = Self {
            theme: config.theme.terminal_theme(),
            custom_chrome,
            initial_size: custom_chrome
                .then(|| egui::vec2(config.window.width, config.window.height)),
            font_size: config.font_size,
            configured_font_size: config.font_size,
            font_dirty: false,
            font_touched_at: Instant::now(),
            bindings: config.keybindings.compile(),
            config,
            config_path,
            tabs,
            active,
            next_tab_id,
            pty_events_tx,
            pty_events_rx,
            config_reload_rx,
            _config_watcher: watcher,
            pending_close: None,
            history: LogStore::new(),
            history_overlay: None,
            closed_tabs: Vec::new(),
            tab_search: None,
            omnibox: None,
            rename_target: None,
            rename_buf: String::new(),
            session_last_saved: Instant::now(),
            session_last_written: session,
            toast: None,
            bell_at: None,
            title_shown: String::new(),
        };
        if let Some(err) = config_error {
            app.set_toast(format!("Config error (using defaults): {err}"));
        }
        Ok(app)
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast {
            msg: msg.into(),
            born: Instant::now(),
            dismissing: false,
        });
    }

    /// A brief accent tint over the window when the terminal rings the bell,
    /// and clearing the taskbar attention flag once focus returns.
    fn show_bell_flash(&mut self, ui: &egui::Ui, ctx: &egui::Context, s: Surfaces) {
        const FLASH: Duration = Duration::from_millis(180);
        let Some(at) = self.bell_at else { return };
        let elapsed = at.elapsed();
        if elapsed >= FLASH {
            self.bell_at = None;
            if ctx.input(|i| i.focused) {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Reset,
                ));
            }
            return;
        }
        let t = 1.0 - elapsed.as_secs_f32() / FLASH.as_secs_f32();
        ui.painter()
            .rect_filled(ctx.content_rect(), 0, s.accent.gamma_multiply(0.22 * t));
        ctx.request_repaint();
    }

    /// Bottom-centered status message, sliding up + fading on both ends.
    fn show_toast(&mut self, ctx: &egui::Context) {
        if let Some(t) = &mut self.toast
            && !t.dismissing
            && t.born.elapsed() >= TOAST_TTL
        {
            t.dismissing = true;
        }

        let want = self.toast.as_ref().is_some_and(|t| !t.dismissing);
        let vis = ctx.animate_bool_with_time(egui::Id::new("pomptty_toast"), want, 0.16);
        if vis == 0.0 {
            if self.toast.as_ref().is_some_and(|t| t.dismissing) {
                self.toast = None;
            }
            return;
        }
        let Some(t) = &self.toast else { return };

        egui::Area::new("pomptty_toast".into())
            .anchor(
                egui::Align2::CENTER_BOTTOM,
                egui::vec2(0.0, -22.0 + (1.0 - vis) * 10.0),
            )
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(vis);
                egui::Frame::popup(ui.style())
                    .corner_radius(8)
                    .show(ui, |ui| ui.label(&t.msg));
            });
    }

    /// Push the active tab's title to the window title bar, if it changed.
    fn sync_window_title(&mut self, ctx: &egui::Context) {
        let want = match self.tabs.get(self.active) {
            Some(tab) if !tab.display_title().is_empty() => {
                format!("{} — pomptty", tab.display_title())
            }
            _ => "pomptty".to_owned(),
        };
        if want != self.title_shown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(want.clone()));
            self.title_shown = want;
        }
    }

    fn active_tab(&mut self) -> &mut TerminalTab {
        &mut self.tabs[self.active]
    }

    /// Open a new tab running the configured shell and switch to it. The new
    /// tab starts in the active tab's working directory.
    fn spawn_tab(&mut self, ctx: &egui::Context) {
        let cwd = self
            .tabs
            .get(self.active)
            .and_then(|t| t.shell_cwd())
            .map(PathBuf::from);
        self.spawn_tab_at(ctx, None, cwd, None, None);
    }

    /// Open a new tab and switch to it. `index` places it at that slot
    /// (clamped to the tab count; `None` appends). `manual_title`/`color`, if
    /// given, are applied to the new tab — used by duplicate / reopen-closed.
    fn spawn_tab_at(
        &mut self,
        ctx: &egui::Context,
        index: Option<usize>,
        cwd: Option<PathBuf>,
        manual_title: Option<String>,
        color: Option<TabColor>,
    ) {
        match TerminalTab::new(
            self.next_tab_id,
            ctx.clone(),
            self.pty_events_tx.clone(),
            self.config.shell.clone(),
            self.config.shell_args.clone(),
            cwd,
        ) {
            Ok(mut tab) => {
                tab.manual_title = manual_title;
                tab.color = color;
                log::info!("opened tab {}", tab.id);
                let at = insert_slot(index, self.tabs.len());
                self.tabs.insert(at, tab);
                self.active = at;
                self.next_tab_id += 1;
            }
            Err(e) => self.set_toast(format!("Could not open a new tab: {e:#}")),
        }
    }

    /// Reopen the most recently closed tab in its old slot and directory, or
    /// open a plain new tab when nothing has been closed.
    fn reopen_tab(&mut self, ctx: &egui::Context) {
        match self.closed_tabs.pop() {
            Some(c) => self.spawn_tab_at(
                ctx,
                Some(c.index),
                c.cwd.map(PathBuf::from),
                Some(c.title),
                c.color,
            ),
            None => self.spawn_tab(ctx),
        }
    }

    /// Reopen closed tab `k` counting from the most recent (`0` = last closed),
    /// as shown in the context menu's "Reopen closed" submenu.
    fn reopen_closed_tab(&mut self, ctx: &egui::Context, k: usize) {
        let Some(slot) = closed_tab_slot(k, self.closed_tabs.len()) else {
            return;
        };
        let c = self.closed_tabs.remove(slot);
        self.spawn_tab_at(
            ctx,
            Some(c.index),
            c.cwd.map(PathBuf::from),
            Some(c.title),
            c.color,
        );
    }

    /// Open a copy of tab `idx` (same directory, title and color) right after it.
    fn duplicate_tab(&mut self, ctx: &egui::Context, idx: usize) {
        let Some(tab) = self.tabs.get(idx) else {
            return;
        };
        let cwd = tab.shell_cwd().map(PathBuf::from);
        let title = tab.manual_title.clone();
        let color = tab.color;
        self.spawn_tab_at(ctx, Some(idx + 1), cwd, title, color);
    }

    /// Close every tab except `idx`, leaving any with a running child alone.
    fn close_other_tabs(&mut self, idx: usize, ctx: &egui::Context) {
        let Some(keep_id) = self.tabs.get(idx).map(|t| t.id) else {
            return;
        };
        let mut kept_busy = 0;
        // Walk backwards so removing a tab never invalidates an index still to
        // come — everything before `i` is untouched by removing at `i`.
        for i in (0..self.tabs.len()).rev() {
            if self.tabs[i].id == keep_id {
                continue;
            }
            if self.tabs[i].has_running_child() {
                kept_busy += 1;
                continue;
            }
            self.close_tab(i, ctx);
        }
        if let Some(pos) = self.tabs.iter().position(|t| t.id == keep_id) {
            self.active = pos;
        }
        if kept_busy > 0 {
            let s = if kept_busy == 1 { "" } else { "s" };
            self.set_toast(format!("Kept {kept_busy} tab{s} with a running process"));
        }
    }

    /// Move the tab at `from` to index `to` (drag-to-reorder), keeping the
    /// active tab selected wherever it lands.
    fn move_tab(&mut self, from: usize, to: usize) {
        let n = self.tabs.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active = remap_index(self.active, from, to);
    }

    /// Close tab `idx`, but if a process is still running in it, raise the
    /// confirmation dialog instead of closing right away.
    fn request_close_tab(&mut self, idx: usize, ctx: &egui::Context) {
        match self.tabs.get(idx) {
            Some(tab) if tab.has_running_child() => self.pending_close = Some(tab.id),
            Some(_) => self.close_tab(idx, ctx),
            None => {}
        }
    }

    /// Close tab `idx`. Closing the last tab quits the app.
    fn close_tab(&mut self, idx: usize, ctx: &egui::Context) {
        if idx >= self.tabs.len() {
            return;
        }
        let title = self.tabs[idx].display_title().to_owned();
        let cwd = self.tabs[idx].shell_cwd();
        let color = self.tabs[idx].color;
        self.closed_tabs.push(ClosedTab {
            title,
            cwd,
            color,
            index: idx,
        });
        if self.closed_tabs.len() > CLOSED_TABS_CAP {
            self.closed_tabs.remove(0);
        }

        let closed = self.tabs.remove(idx); // Drop shuts the PTY down.
        log::info!("closed tab {}", closed.id);
        if self.tabs.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.active > idx {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len() - 1);
    }

    /// Draw the "a process is still running" dialog for `self.pending_close`
    /// and act on the user's choice. The tab is looked up by id each frame, so
    /// it is fine if it disappeared (its shell exited) while the dialog was up.
    fn show_close_confirmation(&mut self, ctx: &egui::Context) {
        let Some(id) = self.pending_close else { return };
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            self.pending_close = None;
            return;
        };
        // The process may have finished (either between the keystroke and this
        // first frame, or while the dialog sat open). Nothing left to warn
        // about, so just carry out the close the user asked for.
        if !self.tabs[idx].has_running_child() {
            self.pending_close = None;
            self.close_tab(idx, ctx);
            return;
        }
        let title = self.tabs[idx].display_title().to_owned();

        let s = Surfaces::from_theme(&self.config.theme);
        let mut close = false;
        let mut cancel = false;
        let vis = ctx.animate_bool_with_time(egui::Id::new("pomptty_confirm_anim"), true, 0.11);
        let shadow_alpha = if self.config.theme.is_dark() { 130 } else { 55 };
        let frame = egui::Frame::new()
            .fill(s.raised)
            .stroke(egui::Stroke::new(1.0, s.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(20))
            .shadow(egui::Shadow {
                offset: [0, 12],
                blur: 34,
                spread: 0,
                color: egui::Color32::from_black_alpha(shadow_alpha),
            });
        let modal = egui::Modal::new(egui::Id::new("pomptty_confirm_close"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_opacity(vis);
                ui.set_width(340.0);
                ui.label(egui::RichText::new("Close this tab?").size(16.0).strong());
                ui.add_space(8.0);
                let what = if title.is_empty() {
                    "A process is still running in this tab.".to_owned()
                } else {
                    format!("“{title}” is still running in this tab.")
                };
                ui.label(
                    egui::RichText::new(format!("{what} Closing it ends that process."))
                        .color(s.text_muted),
                );
                ui.add_space(18.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let danger = egui::Button::new(
                        egui::RichText::new("Close tab").color(crate::ui::style::on_accent(s.err)),
                    )
                    .fill(s.err)
                    .corner_radius(7);
                    if ui.add(danger).clicked() {
                        close = true;
                    }
                    if ui
                        .add(egui::Button::new("Cancel").fill(egui::Color32::TRANSPARENT))
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });

        if close {
            self.pending_close = None;
            self.close_tab(idx, ctx);
        } else if cancel || modal.should_close() {
            self.pending_close = None;
        } else {
            // Poll so the dialog can notice the process finishing on its own.
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    /// Draw the rename dialog for `self.rename_target` and act on the choice.
    fn show_rename_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.rename_target else { return };
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            self.rename_target = None;
            return;
        };

        let s = Surfaces::from_theme(&self.config.theme);
        let mut commit = false;
        let mut cancel = false;
        let vis = ctx.animate_bool_with_time(egui::Id::new("pomptty_rename_anim"), true, 0.11);
        let shadow_alpha = if self.config.theme.is_dark() { 130 } else { 55 };
        let frame = egui::Frame::new()
            .fill(s.raised)
            .stroke(egui::Stroke::new(1.0, s.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(20))
            .shadow(egui::Shadow {
                offset: [0, 12],
                blur: 34,
                spread: 0,
                color: egui::Color32::from_black_alpha(shadow_alpha),
            });
        let modal = egui::Modal::new(egui::Id::new("pomptty_rename"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_opacity(vis);
                ui.set_width(320.0);
                ui.label(egui::RichText::new("Rename tab").size(16.0).strong());
                ui.add_space(10.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.rename_buf)
                        .desired_width(f32::INFINITY)
                        .hint_text("Empty resets to the shell's title"),
                );
                // Check for the commit *before* possibly re-requesting focus:
                // pressing Enter also makes a singleline edit lose focus, and
                // re-granting it here would mask that transition.
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if !resp.has_focus() && !enter {
                    resp.request_focus();
                }
                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let primary = egui::Button::new(
                        egui::RichText::new("Rename").color(crate::ui::style::on_accent(s.accent)),
                    )
                    .fill(s.accent)
                    .corner_radius(7);
                    if ui.add(primary).clicked() || enter {
                        commit = true;
                    }
                    if ui
                        .add(egui::Button::new("Cancel").fill(egui::Color32::TRANSPARENT))
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });

        if commit {
            self.rename_target = None;
            let buf = self.rename_buf.trim();
            self.tabs[idx].manual_title = (!buf.is_empty()).then(|| buf.to_owned());
        } else if cancel || modal.should_close() {
            self.rename_target = None;
        }
    }

    /// Open the `Ctrl+Shift+A` tab switcher over a snapshot of the open tabs.
    fn open_tab_search(&mut self) {
        let entries: Vec<TabEntry> = self
            .tabs
            .iter()
            .map(|t| TabEntry {
                id: t.id,
                title: t.display_title().to_owned(),
                cwd: t.shell_cwd(),
            })
            .collect();
        let active_id = self.tabs[self.active].id;
        self.tab_search = Some(TabSearchOverlay::new(entries, active_id));
    }

    /// Open the `Ctrl+Shift+P` command palette over a snapshot of the open
    /// tabs and recent history.
    fn open_omnibox(&mut self) {
        self.history.refresh();
        let entries: Vec<TabEntry> = self
            .tabs
            .iter()
            .map(|t| TabEntry {
                id: t.id,
                title: t.display_title().to_owned(),
                cwd: t.shell_cwd(),
            })
            .collect();
        let active_id = self.tabs[self.active].id;
        let active_cwd = self.tabs[self.active].shell_cwd();
        // Recent directories, minus the one the active tab is already in.
        let dirs: Vec<String> = self
            .history
            .recent_dirs(20)
            .into_iter()
            .filter(|d| Some(d) != active_cwd.as_ref())
            .collect();
        self.omnibox = Some(OmniboxOverlay::new(entries, active_id, dirs));
    }

    /// Move focus `delta` tabs, wrapping around.
    fn focus_delta(&mut self, delta: isize) {
        let n = self.tabs.len();
        if n <= 1 {
            return;
        }
        self.active = (self.active as isize + delta).rem_euclid(n as isize) as usize;
    }

    /// Jump to 1-based tab `n`; values past the end select the last tab.
    fn goto_tab(&mut self, n: u8) {
        if self.tabs.is_empty() {
            return;
        }
        self.active = (n as usize).saturating_sub(1).min(self.tabs.len() - 1);
    }

    /// Drain PTY events. Returns `true` if the app should close.
    fn pump_pty_events(&mut self, ctx: &egui::Context) -> bool {
        while let Ok((id, event)) = self.pty_events_rx.try_recv() {
            match event {
                PtyEvent::Exit => {
                    self.tabs.retain(|t| t.id != id);
                    if self.tabs.is_empty() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return true;
                    }
                    self.active = self.active.min(self.tabs.len() - 1);
                }
                PtyEvent::Title(title) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.title = title;
                    }
                }
                PtyEvent::Bell => {
                    self.bell_at = Some(Instant::now());
                    if !ctx.input(|i| i.focused) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Informational,
                        ));
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn pump_config_reload(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while self.config_reload_rx.try_recv().is_ok() {
            changed = true;
        }
        if changed {
            self.reload_config(ctx, false);
        }
    }

    /// Reload `config.json`. `announce` controls whether a no-op reload (the
    /// common case for our own font-size write-backs coming back through the
    /// file watcher) still shows a toast — it does only for the manual
    /// `Ctrl+Shift+R`.
    fn reload_config(&mut self, ctx: &egui::Context, announce: bool) {
        let cfg = match Config::load_or_create(&self.config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                self.set_toast(format!("Config error (kept previous): {e:#}"));
                return;
            }
        };

        let same = serde_json::to_string(&cfg).ok() == serde_json::to_string(&self.config).ok();
        if same {
            if announce {
                self.set_toast("Config unchanged");
            }
            return;
        }

        self.theme = cfg.theme.terminal_theme();
        self.configured_font_size = cfg.font_size;
        self.font_size = cfg.font_size;
        self.font_dirty = false;
        self.bindings = cfg.keybindings.compile();
        crate::fonts::apply(ctx, &cfg);
        crate::ui::style::apply(ctx, &cfg.theme);
        self.config = cfg;
        self.set_toast("Config reloaded");
    }

    /// Move the terminal font size one zoom step in `dir` (+1 / -1), snapping to
    /// the next size at which the terminal's integer cell metrics actually
    /// change — otherwise a single press can leave the grid looking unchanged.
    fn zoom_font(&mut self, ctx: &egui::Context, dir: f32) {
        let next = snap_zoom(self.font_size, dir, |pt| {
            ctx.fonts_mut(|f| f.glyph_width(&egui::FontId::monospace(pt), 'm'))
                .floor()
        });
        self.set_font_size(next);
    }

    fn set_font_size(&mut self, pt: f32) {
        let pt = pt.clamp(FONT_MIN, FONT_MAX);
        if pt != self.font_size {
            self.font_size = pt;
            self.font_dirty = true;
        }
        self.font_touched_at = Instant::now();
    }

    /// Once the font size has been stable for [`FONT_PERSIST_DELAY`], write it
    /// back to `config.json` so it survives a restart.
    fn persist_font_size_when_settled(&mut self, ctx: &egui::Context) {
        if !self.font_dirty {
            return;
        }
        match FONT_PERSIST_DELAY.checked_sub(self.font_touched_at.elapsed()) {
            Some(remaining) => ctx.request_repaint_after(remaining),
            None => {
                self.font_dirty = false;
                if self.config.font_size != self.font_size {
                    self.config.font_size = self.font_size;
                    if let Err(e) = self.config.save(&self.config_path) {
                        log::warn!("could not persist font size: {e:#}");
                    }
                }
            }
        }
    }

    /// Check the open tabs against the last saved session every
    /// [`SESSION_SAVE_INTERVAL`], writing only when something changed (a tab
    /// opened/closed/moved/renamed, or one `cd`'d somewhere new).
    fn maybe_persist_session(&mut self, ctx: &egui::Context) {
        if !self.config.session.restore {
            return;
        }
        match SESSION_SAVE_INTERVAL.checked_sub(self.session_last_saved.elapsed()) {
            Some(remaining) => ctx.request_repaint_after(remaining),
            None => {
                self.session_last_saved = Instant::now();
                let snapshot = Session::capture(&self.tabs, self.active);
                if self.session_last_written.as_ref() != Some(&snapshot) {
                    if let Err(e) = snapshot.save() {
                        log::warn!("could not save session: {e:#}");
                    }
                    self.session_last_written = Some(snapshot);
                }
                ctx.request_repaint_after(SESSION_SAVE_INTERVAL);
            }
        }
    }

    /// Open the `Ctrl+R` history overlay. With history disabled in the config,
    /// forward a real `Ctrl+R` to the shell instead so its own reverse-i-search
    /// still works.
    fn open_history_search(&mut self) {
        if !self.config.history.enabled {
            self.active_tab().write(vec![0x12]); // Ctrl+R
            return;
        }
        self.history.refresh();
        let cwd = self.active_tab().shell_cwd();
        self.history_overlay = Some(HistoryOverlay::new(cwd));
    }

    fn handle_bindings(&mut self, ctx: &egui::Context) {
        let mut hits: Vec<Action> = Vec::new();
        ctx.input_mut(|input| {
            for (chord, action) in &self.bindings {
                if action.is_active() && input.consume_key(chord.modifiers, chord.key) {
                    hits.push(*action);
                }
            }
        });
        for action in hits {
            self.dispatch(action, ctx);
        }
    }

    fn dispatch(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::FontIncrease => self.zoom_font(ctx, 1.0),
            Action::FontDecrease => self.zoom_font(ctx, -1.0),
            Action::FontReset => self.set_font_size(self.configured_font_size),
            Action::ScrollPageUp => self.active_tab().scroll(PAGE_LINES),
            Action::ScrollPageDown => self.active_tab().scroll(-PAGE_LINES),
            Action::ScrollToTop => self.active_tab().scroll(SCROLL_TO_EDGE),
            Action::ScrollToBottom => self.active_tab().scroll(-SCROLL_TO_EDGE),
            Action::Clear => self.active_tab().write(vec![0x0c]), // Ctrl+L
            Action::ReloadConfig => self.reload_config(ctx, true),
            Action::NewTab => self.spawn_tab(ctx),
            Action::ReopenTab => self.reopen_tab(ctx),
            Action::CloseTab => self.request_close_tab(self.active, ctx),
            Action::NextTab => self.focus_delta(1),
            Action::PrevTab => self.focus_delta(-1),
            Action::GotoTab(n) => self.goto_tab(n),
            Action::TabSearch => self.open_tab_search(),
            Action::HistorySearch => self.open_history_search(),
            Action::Omnibox => self.open_omnibox(),
            // Inert here: handled by the terminal widget, or filtered out before
            // dispatch (see `Action::is_active` and `KeyBindings::compile`).
            Action::Copy | Action::Paste | Action::Disabled => {}
        }
    }
}

impl eframe::App for PompttyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Re-assert the configured window size once — a bare undecorated window
        // is left at whatever size the WM chose on some X11 setups.
        if let Some(size) = self.initial_size.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }

        if self.pump_pty_events(&ctx) {
            return;
        }
        self.pump_config_reload(&ctx);
        // While a modal (close confirmation, history search) is up, the keyboard
        // belongs to it.
        if self.pending_close.is_none()
            && self.history_overlay.is_none()
            && self.rename_target.is_none()
            && self.tab_search.is_none()
            && self.omnibox.is_none()
        {
            self.handle_bindings(&ctx);
        }
        if self.tabs.is_empty() {
            return;
        }

        let surfaces = Surfaces::from_theme(&self.config.theme);
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if self.custom_chrome && !maximized {
            resize_edges(ui, &ctx);
        }

        let tab_meta: Vec<(TabId, String, Option<TabColor>)> = self
            .tabs
            .iter()
            .map(|t| (t.id, t.display_title().to_owned(), t.color))
            .collect();
        let tabs: Vec<TabView<'_>> = tab_meta
            .iter()
            .map(|(id, title, color)| TabView {
                id: *id,
                title,
                color: *color,
            })
            .collect();
        let closed_titles: Vec<&str> = self
            .closed_tabs
            .iter()
            .rev()
            .map(|c| c.title.as_str())
            .collect();
        let (chrome_action, chrome_animating) = TabStrip {
            tabs: &tabs,
            active: self.active,
            surfaces,
            window_controls: self.custom_chrome,
            closed: &closed_titles,
        }
        .show(ui);
        if chrome_animating {
            ctx.request_repaint();
        }

        match chrome_action {
            ChromeAction::NewTab => self.spawn_tab(&ctx),
            ChromeAction::SelectTab(i) => {
                if i < self.tabs.len() {
                    self.active = i;
                }
            }
            ChromeAction::CloseTab(i) => self.request_close_tab(i, &ctx),
            ChromeAction::MoveTab { from, to } => self.move_tab(from, to),
            ChromeAction::RenameTab(i) => {
                if let Some(tab) = self.tabs.get(i) {
                    self.rename_target = Some(tab.id);
                    self.rename_buf = tab.display_title().to_owned();
                }
            }
            ChromeAction::DuplicateTab(i) => self.duplicate_tab(&ctx, i),
            ChromeAction::CloseOtherTabs(i) => self.close_other_tabs(i, &ctx),
            ChromeAction::ReopenClosedTab(k) => self.reopen_closed_tab(&ctx, k),
            ChromeAction::SetTabColor(i, color) => {
                if let Some(tab) = self.tabs.get_mut(i) {
                    tab.color = color;
                }
            }
            ChromeAction::OpenSearch => self.open_history_search(),
            ChromeAction::Minimize => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            ChromeAction::ToggleMaximize => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            ChromeAction::CloseWindow => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ChromeAction::BeginWindowDrag => {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            ChromeAction::None => {}
        }
        if self.tabs.is_empty() {
            return;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.sync_window_title(&ctx);
        self.persist_font_size_when_settled(&ctx);
        self.maybe_persist_session(&ctx);

        if self.pending_close.is_some() {
            self.show_close_confirmation(&ctx);
            if self.tabs.is_empty() {
                return;
            }
        } else {
            // Re-prime the modal's open animation for next time.
            ctx.animate_bool_with_time(egui::Id::new("pomptty_confirm_anim"), false, 0.0);
        }

        if self.rename_target.is_some() {
            self.show_rename_dialog(&ctx);
        } else {
            ctx.animate_bool_with_time(egui::Id::new("pomptty_rename_anim"), false, 0.0);
        }

        // Rendered before the other overlays: picking "Search Tabs" / "Search
        // History" from the palette closes it and opens one of them via the
        // normal dispatch, and that should show up this same frame.
        if self.omnibox.is_some() {
            let dark = self.config.theme.is_dark();
            let outcome = self
                .omnibox
                .as_mut()
                .unwrap()
                .show(&ctx, &self.history, surfaces, dark);
            match outcome {
                OmniboxOutcome::None => {}
                OmniboxOutcome::Dismiss => self.omnibox = None,
                OmniboxOutcome::RunAction(action) => {
                    self.omnibox = None;
                    self.dispatch(action, &ctx);
                }
                OmniboxOutcome::SelectTab(id) => {
                    self.omnibox = None;
                    if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
                        self.active = pos;
                    }
                }
                OmniboxOutcome::InsertCommand(cmd) => {
                    self.omnibox = None;
                    self.active_tab().write(cmd.into_bytes());
                }
                OmniboxOutcome::RunCommand(cmd) => {
                    self.omnibox = None;
                    let mut bytes = cmd.into_bytes();
                    bytes.push(b'\r');
                    self.active_tab().write(bytes);
                }
                OmniboxOutcome::ChangeDir(dir) => {
                    self.omnibox = None;
                    let quoted = dir.replace('\'', r"'\''");
                    let mut bytes = format!("cd '{quoted}'").into_bytes();
                    bytes.push(b'\r');
                    self.active_tab().write(bytes);
                }
            }
        }

        if self.tab_search.is_some() {
            let dark = self.config.theme.is_dark();
            let outcome = self.tab_search.as_mut().unwrap().show(&ctx, surfaces, dark);
            match outcome {
                TabSearchOutcome::None => {}
                TabSearchOutcome::Dismiss => self.tab_search = None,
                TabSearchOutcome::Select(id) => {
                    self.tab_search = None;
                    if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
                        self.active = pos;
                    }
                }
            }
        }

        if self.history_overlay.is_some() {
            let dark = self.config.theme.is_dark();
            let max_results = self.config.history.max_results;
            let outcome = self.history_overlay.as_mut().unwrap().show(
                &ctx,
                &self.history,
                max_results,
                surfaces,
                dark,
            );
            match outcome {
                HistoryOutcome::None => {}
                HistoryOutcome::Dismiss => self.history_overlay = None,
                HistoryOutcome::Insert(cmd) => {
                    self.history_overlay = None;
                    self.active_tab().write(cmd.into_bytes());
                }
                HistoryOutcome::Run(cmd) => {
                    self.history_overlay = None;
                    let mut bytes = cmd.into_bytes();
                    bytes.push(b'\r');
                    self.active_tab().write(bytes);
                }
            }
        }

        let panel = egui::Frame::new()
            .fill(surfaces.bg)
            .inner_margin(egui::Margin::same(TERMINAL_MARGIN));
        egui::CentralPanel::default().frame(panel).show(ui, |ui| {
            let tab = &mut self.tabs[self.active];
            let view = TerminalView::new(ui, &mut tab.backend)
                .set_focus(
                    self.pending_close.is_none()
                        && self.history_overlay.is_none()
                        && self.rename_target.is_none()
                        && self.tab_search.is_none()
                        && self.omnibox.is_none(),
                )
                .set_theme(self.theme.clone())
                .set_font(TerminalFont::new(FontSettings {
                    font_type: egui::FontId::monospace(self.font_size),
                }))
                .set_size(ui.available_size());
            ui.add(view);
        });

        if self.custom_chrome {
            // A hairline border so the frameless window has a defined edge.
            let r = ctx.content_rect();
            ui.painter().rect_stroke(
                r.shrink(0.5),
                0,
                egui::Stroke::new(1.0, surfaces.border),
                egui::StrokeKind::Inside,
            );
        }

        self.show_bell_flash(ui, &ctx, surfaces);
        self.show_toast(&ctx);
    }

    /// Called once on shutdown (however it was triggered — the window's ×,
    /// `close-tab` on the last tab, `CloseWindow`). A final, unconditional
    /// save so a clean quit's session is never more than
    /// [`SESSION_SAVE_INTERVAL`] stale. Not called on a hard crash — the
    /// periodic save in [`Self::maybe_persist_session`] is what covers that.
    fn on_exit(&mut self) {
        if !self.config.session.restore {
            return;
        }
        if let Err(e) = Session::capture(&self.tabs, self.active).save() {
            log::warn!("could not save session on exit: {e:#}");
        }
    }
}

/// Invisible 6px hit regions along the window edges/corners that begin a
/// system resize-drag (custom decorations only).
fn resize_edges(ui: &egui::Ui, ctx: &egui::Context) {
    use egui::ResizeDirection as D;
    use egui::{CursorIcon as C, Rect};

    const M: f32 = 6.0;
    let r = ctx.content_rect();
    // (rect, direction, cursor)
    let regions = [
        (
            Rect::from_min_max(r.left_top(), r.right_top() + egui::vec2(0.0, M)),
            D::North,
            C::ResizeNorth,
        ),
        (
            Rect::from_min_max(r.left_bottom() - egui::vec2(0.0, M), r.right_bottom()),
            D::South,
            C::ResizeSouth,
        ),
        (
            Rect::from_min_max(r.left_top(), r.left_bottom() + egui::vec2(M, 0.0)),
            D::West,
            C::ResizeWest,
        ),
        (
            Rect::from_min_max(r.right_top() - egui::vec2(M, 0.0), r.right_bottom()),
            D::East,
            C::ResizeEast,
        ),
        (
            Rect::from_min_max(r.left_top(), r.left_top() + egui::vec2(M, M)),
            D::NorthWest,
            C::ResizeNorthWest,
        ),
        (
            Rect::from_min_max(
                r.right_top() - egui::vec2(M, 0.0),
                r.right_top() + egui::vec2(0.0, M),
            ),
            D::NorthEast,
            C::ResizeNorthEast,
        ),
        (
            Rect::from_min_max(
                r.left_bottom() - egui::vec2(0.0, M),
                r.left_bottom() + egui::vec2(M, 0.0),
            ),
            D::SouthWest,
            C::ResizeSouthWest,
        ),
        (
            Rect::from_min_max(r.right_bottom() - egui::vec2(M, M), r.right_bottom()),
            D::SouthEast,
            C::ResizeSouthEast,
        ),
    ];
    for (i, (rect, dir, cursor)) in regions.into_iter().enumerate() {
        let resp = ui.interact(
            rect,
            egui::Id::new(("pomptty_resize", i)),
            egui::Sense::drag(),
        );
        if resp.hovered() {
            ctx.set_cursor_icon(cursor);
        }
        if resp.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }
}

/// Where a tab requesting slot `index` (`None` = append) actually lands among
/// `len` existing tabs — clamped so a stale or out-of-range request can't panic.
fn insert_slot(index: Option<usize>, len: usize) -> usize {
    index.unwrap_or(len).min(len)
}

/// Position in `closed_tabs` for "the `k`-th most recently closed" (`0` = the
/// last one closed), as shown newest-first in the "Reopen closed" submenu.
/// `None` if there aren't that many.
fn closed_tab_slot(k: usize, len: usize) -> Option<usize> {
    if k >= len { None } else { Some(len - 1 - k) }
}

/// Where index `idx` lands after the element at `from` is removed and
/// re-inserted at `to`. Used to keep the active tab selected across a reorder.
fn remap_index(idx: usize, from: usize, to: usize) -> usize {
    if idx == from {
        to
    } else if from < idx && idx <= to {
        idx - 1
    } else if to <= idx && idx < from {
        idx + 1
    } else {
        idx
    }
}

/// Step `start` in direction `dir` (+/-) in 0.5pt increments until `cell_width`
/// (the terminal's per-cell advance, floored to whole pixels) differs from where
/// it started, or a font bound is hit. This keeps every zoom keypress producing
/// a visible change: `egui_term` floors the cell size to an integer, so a small
/// point change on its own can reflow nothing.
fn snap_zoom(start: f32, dir: f32, cell_width: impl Fn(f32) -> f32) -> f32 {
    let start_cell = cell_width(start);
    let step = 0.5_f32.copysign(dir);
    let mut next = start;
    for _ in 0..280 {
        let cand = (next + step).clamp(FONT_MIN, FONT_MAX);
        if cand == next {
            break; // hit FONT_MIN / FONT_MAX
        }
        next = cand;
        if cell_width(next) != start_cell {
            break;
        }
    }
    next
}

/// Watch the config file's directory and signal `()` on any change to it.
fn spawn_config_watcher(
    config_path: &std::path::Path,
    ctx: egui::Context,
) -> (Receiver<()>, Option<notify::RecommendedWatcher>) {
    let (tx, rx) = channel();
    let watch_dir = config_path.parent().map(PathBuf::from);
    let file_name = config_path.file_name().map(|s| s.to_owned());

    let watcher = (|| -> notify::Result<notify::RecommendedWatcher> {
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                let Ok(event) = res else { return };
                let touches_config = file_name
                    .as_ref()
                    .map(|name| {
                        event
                            .paths
                            .iter()
                            .any(|p| p.file_name() == Some(name.as_os_str()))
                    })
                    .unwrap_or(true);
                let relevant = matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                );
                if touches_config && relevant && tx.send(()).is_ok() {
                    ctx.request_repaint();
                }
            })?;
        if let Some(dir) = &watch_dir {
            watcher.watch(dir, RecursiveMode::NonRecursive)?;
        }
        Ok(watcher)
    })();

    match watcher {
        Ok(w) => (rx, Some(w)),
        Err(e) => {
            log::warn!("config live-reload disabled: {e}");
            (rx, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FONT_MAX, FONT_MIN, closed_tab_slot, insert_slot, remap_index, snap_zoom};

    #[test]
    fn insert_slot_appends_or_clamps() {
        assert_eq!(insert_slot(None, 3), 3, "no index requested -> append");
        assert_eq!(insert_slot(Some(1), 3), 1);
        assert_eq!(insert_slot(Some(3), 3), 3, "at the end is fine");
        assert_eq!(
            insert_slot(Some(99), 3),
            3,
            "a stale index clamps to append"
        );
        assert_eq!(insert_slot(None, 0), 0);
    }

    #[test]
    fn closed_tab_slot_counts_back_from_the_most_recent() {
        // 3 closed tabs, oldest at index 0: [old, mid, new].
        assert_eq!(closed_tab_slot(0, 3), Some(2), "most recent");
        assert_eq!(closed_tab_slot(1, 3), Some(1));
        assert_eq!(closed_tab_slot(2, 3), Some(0), "oldest");
        assert_eq!(closed_tab_slot(3, 3), None, "nothing that far back");
        assert_eq!(closed_tab_slot(0, 0), None, "nothing closed yet");
    }

    #[test]
    fn remap_index_tracks_the_active_tab_across_a_reorder() {
        // Drag tab 0 to slot 2: [A B C D] -> [B C A D].
        assert_eq!(remap_index(0, 0, 2), 2); // the dragged tab itself
        assert_eq!(remap_index(1, 0, 2), 0); // B shifts left
        assert_eq!(remap_index(2, 0, 2), 1); // C shifts left
        assert_eq!(remap_index(3, 0, 2), 3); // D untouched

        // Drag tab 3 to slot 1: [A B C D] -> [A D B C].
        assert_eq!(remap_index(3, 3, 1), 1);
        assert_eq!(remap_index(1, 3, 1), 2);
        assert_eq!(remap_index(2, 3, 1), 3);
        assert_eq!(remap_index(0, 3, 1), 0);

        // No-op.
        assert_eq!(remap_index(2, 2, 2), 2);
    }

    /// A stand-in for a monospace face: ~0.6pt of advance per point, floored to
    /// whole pixels the way `egui_term` does.
    fn cell(pt: f32) -> f32 {
        (pt * 0.6).floor()
    }

    #[test]
    fn snap_zoom_lands_on_a_real_grid_change() {
        let up = snap_zoom(14.0, 1.0, cell);
        assert!(up > 14.0);
        assert_ne!(cell(up), cell(14.0));

        let down = snap_zoom(14.0, -1.0, cell);
        assert!(down < 14.0);
        assert_ne!(cell(down), cell(14.0));
    }

    #[test]
    fn snap_zoom_takes_the_smallest_step_that_shows() {
        // From 14 (cell 8), the first 0.5 step that changes the floored cell is
        // 15.0 (cell 9) — not 14.5 (still 8).
        assert_eq!(cell(14.5), cell(14.0));
        assert_eq!(snap_zoom(14.0, 1.0, cell), 15.0);
    }

    #[test]
    fn snap_zoom_stops_at_the_bounds() {
        let never = |_pt: f32| 0.0;
        assert_eq!(snap_zoom(FONT_MAX, 1.0, never), FONT_MAX);
        assert_eq!(snap_zoom(FONT_MIN, -1.0, never), FONT_MIN);
        // Even a size that never reflows must not run past the ceiling.
        assert_eq!(snap_zoom(14.0, 1.0, never), FONT_MAX);
    }
}
