//! One terminal pane: a PTY-backed `egui_term` session plus its display state.
//! The app owns a `Vec` of these, one per tab.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use anyhow::{Context, Result};
use egui_term::{BackendCommand, BackendSettings, CursorShape, PtyEvent, TerminalBackend};
use serde::{Deserialize, Serialize};

/// Identifier for a tab. Monotonic; never reused within a run.
pub type TabId = u64;

/// A user-assigned tab color (context-menu "Color"), purely a chrome marker —
/// fixed, vivid swatches independent of the active theme, like Chrome's
/// tab-group colors, so a color reads the same regardless of palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabColor {
    Grey,
    Blue,
    Red,
    Yellow,
    Green,
    Pink,
    Purple,
    Cyan,
}

impl TabColor {
    pub const ALL: [TabColor; 8] = [
        TabColor::Grey,
        TabColor::Blue,
        TabColor::Red,
        TabColor::Yellow,
        TabColor::Green,
        TabColor::Pink,
        TabColor::Purple,
        TabColor::Cyan,
    ];

    pub fn rgb(self) -> egui::Color32 {
        match self {
            TabColor::Grey => egui::Color32::from_rgb(0x9a, 0xa0, 0xa6),
            TabColor::Blue => egui::Color32::from_rgb(0x1a, 0x73, 0xe8),
            TabColor::Red => egui::Color32::from_rgb(0xd9, 0x30, 0x25),
            TabColor::Yellow => egui::Color32::from_rgb(0xf9, 0xab, 0x00),
            TabColor::Green => egui::Color32::from_rgb(0x18, 0x8f, 0x39),
            TabColor::Pink => egui::Color32::from_rgb(0xd0, 0x1d, 0x84),
            TabColor::Purple => egui::Color32::from_rgb(0x8f, 0x3d, 0xe8),
            TabColor::Cyan => egui::Color32::from_rgb(0x12, 0xa4, 0xaf),
        }
    }
}

pub struct TerminalTab {
    pub id: TabId,
    /// The title the shell last set (OSC 0/2).
    pub title: String,
    /// A user-set name (context-menu "Rename"). Wins over `title` until cleared.
    pub manual_title: Option<String>,
    /// A user-assigned color (context-menu "Color"), shown as a stripe on the
    /// tab. Purely cosmetic.
    pub color: Option<TabColor>,
    pub backend: TerminalBackend,
}

impl TerminalTab {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: TabId,
        ctx: egui::Context,
        pty_events: Sender<(TabId, PtyEvent)>,
        shell: Option<String>,
        shell_args: Vec<String>,
        cwd: Option<PathBuf>,
        cursor_shape: CursorShape,
        cursor_blinking: bool,
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
                working_directory: cwd.filter(|p| p.is_dir()),
                cursor_shape,
                cursor_blinking,
            },
        )
        .context("failed to start the shell process")?;

        Ok(Self {
            id,
            title: format!("Terminal {id}"),
            manual_title: None,
            color: None,
            backend,
        })
    }

    /// The name to show for this tab: the user's override if set, else the
    /// shell-set title.
    pub fn display_title(&self) -> &str {
        self.manual_title.as_deref().unwrap_or(&self.title)
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

    /// Whether a process other than the shell itself is running in this tab
    /// (an editor, an `ssh` session, a build, …). Used to warn before closing
    /// the tab. Linux-only; returns `false` where `/proc` is unavailable.
    pub fn has_running_child(&self) -> bool {
        has_child_process(self.backend.pty_id())
    }

    /// The shell's current working directory, from `/proc/<pid>/cwd`. Used to
    /// scope history search to "this directory". Linux-only; `None` elsewhere or
    /// if the shell has exited.
    pub fn shell_cwd(&self) -> Option<String> {
        shell_cwd(self.backend.pty_id())
    }
}

#[cfg(target_os = "linux")]
fn shell_cwd(shell_pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{shell_pid}/cwd"))
        .ok()?
        .to_str()
        .map(str::to_owned)
}

#[cfg(not(target_os = "linux"))]
fn shell_cwd(_shell_pid: u32) -> Option<String> {
    None
}

/// True if any live (non-zombie) process has `shell_pid` as its parent.
#[cfg(target_os = "linux")]
fn has_child_process(shell_pid: u32) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some((ppid, state)) = parse_stat_ppid_state(&stat)
            && ppid == shell_pid
            && state != 'Z'
        {
            return true;
        }
    }
    false
}

/// Pull `(ppid, state)` out of a `/proc/<pid>/stat` line. The format is
/// `pid (comm) state ppid …`; `comm` can itself contain spaces and parens, so
/// the fields are read from after the final ')'.
#[cfg(target_os = "linux")]
fn parse_stat_ppid_state(stat: &str) -> Option<(u32, char)> {
    let rest = stat.rsplit_once(')')?.1;
    let mut fields = rest.split_whitespace();
    let state = fields.next()?.chars().next()?;
    let ppid = fields.next()?.parse().ok()?;
    Some((ppid, state))
}

#[cfg(not(target_os = "linux"))]
fn has_child_process(_shell_pid: u32) -> bool {
    false
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::parse_stat_ppid_state;

    #[test]
    fn parses_a_plain_stat_line() {
        let line = "4242 (bash) S 4200 4242 4242 34816 4300 4194304 …";
        assert_eq!(parse_stat_ppid_state(line), Some((4200, 'S')));
    }

    #[test]
    fn handles_comm_with_spaces_and_parens() {
        let line = "915 (Web Content (tab)) R 880 915 880 0 -1 …";
        assert_eq!(parse_stat_ppid_state(line), Some((880, 'R')));
    }

    #[test]
    fn recognizes_a_zombie() {
        let line = "77 (defunct-thing) Z 40 77 40 0 -1 …";
        assert_eq!(parse_stat_ppid_state(line), Some((40, 'Z')));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_stat_ppid_state("not a stat line"), None);
    }
}

#[cfg(test)]
mod tab_color_tests {
    use super::TabColor;

    #[test]
    fn every_swatch_is_distinct() {
        let rgbs: Vec<_> = TabColor::ALL.iter().map(|c| c.rgb()).collect();
        for (i, a) in rgbs.iter().enumerate() {
            for (j, b) in rgbs.iter().enumerate() {
                assert!(i == j || a != b, "duplicate swatch: {:?}", TabColor::ALL[i]);
            }
        }
    }
}
