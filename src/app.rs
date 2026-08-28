//! The eframe application: owns the tabs, config, and the frame around them.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::Result;
use egui_term::{FontSettings, PtyEvent, TerminalFont, TerminalTheme, TerminalView};
use notify::{RecursiveMode, Watcher};

use crate::config::Config;
use crate::config::keybindings::{Action, Chord};
use crate::terminal::{TabId, TerminalTab};
use crate::ui::chrome::{ChromeAction, TabStrip};

/// How many lines a "page" scroll moves. A rough constant is fine — the backend
/// clamps at the ends of the scrollback.
const PAGE_LINES: i32 = 24;
const SCROLL_TO_EDGE: i32 = 1_000_000;
const TOAST_TTL: Duration = Duration::from_secs(5);

pub struct PompttyApp {
    config: Config,
    config_path: PathBuf,
    theme: TerminalTheme,
    /// Current (possibly zoomed) font size, and the size to reset to.
    font_size: f32,
    base_font_size: f32,

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
    toast: Option<(String, Instant)>,
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
        ctx.set_visuals(config.theme.egui_visuals());

        let (pty_events_tx, pty_events_rx) = channel();
        let first = TerminalTab::new(
            0,
            ctx.clone(),
            pty_events_tx.clone(),
            config.shell.clone(),
            config.shell_args.clone(),
        )?;

        let (config_reload_rx, watcher) = spawn_config_watcher(&config_path, ctx.clone());

        let mut app = Self {
            theme: config.theme.terminal_theme(),
            font_size: config.font_size,
            base_font_size: config.font_size,
            bindings: config.keybindings.compile(),
            config,
            config_path,
            tabs: vec![first],
            active: 0,
            next_tab_id: 1,
            pty_events_tx,
            pty_events_rx,
            config_reload_rx,
            _config_watcher: watcher,
            toast: None,
            title_shown: String::new(),
        };
        if let Some(err) = config_error {
            app.set_toast(format!("Config error (using defaults): {err}"));
        }
        Ok(app)
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    /// Push the active tab's title to the window title bar, if it changed.
    fn sync_window_title(&mut self, ctx: &egui::Context) {
        let want = match self.tabs.get(self.active) {
            Some(tab) if !tab.title.is_empty() => format!("{} — pomptty", tab.title),
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

    /// Open a new tab running the configured shell and switch to it.
    fn spawn_tab(&mut self, ctx: &egui::Context) {
        match TerminalTab::new(
            self.next_tab_id,
            ctx.clone(),
            self.pty_events_tx.clone(),
            self.config.shell.clone(),
            self.config.shell_args.clone(),
        ) {
            Ok(tab) => {
                log::info!("opened tab {}", tab.id);
                self.tabs.push(tab);
                self.active = self.tabs.len() - 1;
                self.next_tab_id += 1;
            }
            Err(e) => self.set_toast(format!("Could not open a new tab: {e:#}")),
        }
    }

    /// Close tab `idx`. Closing the last tab quits the app.
    fn close_tab(&mut self, idx: usize, ctx: &egui::Context) {
        if idx >= self.tabs.len() {
            return;
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
            self.reload_config(ctx);
        }
    }

    fn reload_config(&mut self, ctx: &egui::Context) {
        match Config::load_or_create(&self.config_path) {
            Ok(cfg) => {
                self.theme = cfg.theme.terminal_theme();
                self.base_font_size = cfg.font_size;
                self.font_size = cfg.font_size;
                self.bindings = cfg.keybindings.compile();
                crate::fonts::apply(ctx, &cfg);
                ctx.set_visuals(cfg.theme.egui_visuals());
                self.config = cfg;
                self.set_toast("Config reloaded");
            }
            Err(e) => self.set_toast(format!("Config error (kept previous): {e:#}")),
        }
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
            Action::FontIncrease => self.font_size = (self.font_size + 1.0).min(72.0),
            Action::FontDecrease => self.font_size = (self.font_size - 1.0).max(4.0),
            Action::FontReset => self.font_size = self.base_font_size,
            Action::ScrollPageUp => self.active_tab().scroll(PAGE_LINES),
            Action::ScrollPageDown => self.active_tab().scroll(-PAGE_LINES),
            Action::ScrollToTop => self.active_tab().scroll(SCROLL_TO_EDGE),
            Action::ScrollToBottom => self.active_tab().scroll(-SCROLL_TO_EDGE),
            Action::Clear => self.active_tab().write(vec![0x0c]), // Ctrl+L
            Action::ReloadConfig => self.reload_config(ctx),
            Action::NewTab => self.spawn_tab(ctx),
            Action::CloseTab => self.close_tab(self.active, ctx),
            Action::NextTab => self.focus_delta(1),
            Action::PrevTab => self.focus_delta(-1),
            Action::GotoTab(n) => self.goto_tab(n),
            // Inert in this milestone (see `Action::is_active`).
            Action::Copy | Action::Paste | Action::HistorySearch => {}
        }
    }
}

impl eframe::App for PompttyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.pump_pty_events(&ctx) {
            return;
        }
        self.pump_config_reload(&ctx);
        self.handle_bindings(&ctx);
        if self.tabs.is_empty() {
            return;
        }

        let palette = self.config.theme.palette();
        let accent = color32(&palette.blue);
        let muted = color32(&palette.bright_black);

        let titles: Vec<String> = self.tabs.iter().map(|t| t.title.clone()).collect();
        let chrome_action = TabStrip {
            titles: &titles,
            active: self.active,
            accent,
            muted,
        }
        .show(ui);
        match chrome_action {
            ChromeAction::NewTab => self.spawn_tab(&ctx),
            ChromeAction::SelectTab(i) => {
                if i < self.tabs.len() {
                    self.active = i;
                }
            }
            ChromeAction::CloseTab(i) => self.close_tab(i, &ctx),
            ChromeAction::OpenSearch => {
                self.set_toast("History search (Ctrl+R) is not implemented yet");
            }
            ChromeAction::None => {}
        }
        if self.tabs.is_empty() {
            return;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.sync_window_title(&ctx);

        egui::CentralPanel::default().show(ui, |ui| {
            let tab = &mut self.tabs[self.active];
            let view = TerminalView::new(ui, &mut tab.backend)
                .set_focus(true)
                .set_theme(self.theme.clone())
                .set_font(TerminalFont::new(FontSettings {
                    font_type: egui::FontId::monospace(self.font_size),
                }))
                .set_size(ui.available_size());
            ui.add(view);
        });

        let expired = matches!(&self.toast, Some((_, t)) if t.elapsed() >= TOAST_TTL);
        if expired {
            self.toast = None;
        }
        if let Some((msg, _)) = &self.toast {
            egui::Area::new("pomptty_toast".into())
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -18.0))
                .interactable(false)
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(msg);
                    });
                });
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn color32(hex: &str) -> egui::Color32 {
    let [r, g, b] = crate::config::theme::parse_hex(hex).unwrap_or([128, 128, 128]);
    egui::Color32::from_rgb(r, g, b)
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
