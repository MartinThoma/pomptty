//! One terminal pane: a PTY-backed `egui_term` session plus its display state.
//!
//! Milestone 1 renders exactly one of these, but the app already owns a `Vec`
//! of them so the tabs milestone is additive.

use std::sync::mpsc::Sender;

use anyhow::{Context, Result};
use egui_term::{BackendCommand, BackendSettings, PtyEvent, TerminalBackend};

/// Identifier for a tab. Monotonic; never reused within a run.
pub type TabId = u64;

pub struct TerminalTab {
    pub id: TabId,
    pub title: String,
    pub backend: TerminalBackend,
}

impl TerminalTab {
    pub fn new(
        id: TabId,
        ctx: egui::Context,
        pty_events: Sender<(TabId, PtyEvent)>,
        shell: Option<String>,
        shell_args: Vec<String>,
    ) -> Result<Self> {
        let shell = shell
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| {
                log::warn!("no shell configured and $SHELL is unset; falling back to /bin/bash");
                "/bin/bash".to_owned()
            });

        let backend = TerminalBackend::new(
            id,
            ctx,
            pty_events,
            BackendSettings {
                shell,
                args: shell_args,
                ..Default::default()
            },
        )
        .context("failed to start the shell process")?;

        Ok(Self {
            id,
            title: format!("Terminal {id}"),
            backend,
        })
    }

    /// Scroll the viewport by `lines` (positive = towards history).
    pub fn scroll(&mut self, lines: i32) {
        self.backend.process_command(BackendCommand::Scroll(lines));
    }

    /// Write raw bytes to the PTY (used by the history feature to insert a
    /// recalled command).
    pub fn write(&mut self, bytes: Vec<u8>) {
        self.backend.process_command(BackendCommand::Write(bytes));
    }
}
