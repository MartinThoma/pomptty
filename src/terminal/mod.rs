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
    /// Whether the shell — or something running under it (`sudo -s`, `su`, a
    /// long `sudo …`) — is `root`. Drives the red superuser warning. Refreshed
    /// on a poll by the app; `false` outside Linux / macOS.
    pub is_root: bool,
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
        osc52_read: bool,
    ) -> Result<Self> {
        let shell = shell
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(default_shell);

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
                osc52_read,
            },
        )
        .context("failed to start the shell process")?;

        Ok(Self {
            id,
            title: format!("Terminal {id}"),
            manual_title: None,
            color: None,
            is_root: false,
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

    /// Write a protocol reply (OSC 52 clipboard read, OSC 10/11/12 colour
    /// query) to the PTY without scrolling the viewport.
    pub fn report(&mut self, bytes: Vec<u8>) {
        self.backend.process_command(BackendCommand::Report(bytes));
    }

    /// The active tab's whole grid + scrollback as plain text.
    pub fn scrollback_text(&self) -> String {
        self.backend.scrollback_text()
    }

    /// Whether a process other than the shell itself is running in this tab
    /// (an editor, an `ssh` session, a build, …). Used to warn before closing
    /// the tab. Linux (`/proc`) and macOS (`ps`); `false` elsewhere.
    pub fn has_running_child(&self) -> bool {
        has_child_process(self.backend.pty_id())
    }

    /// The shell's current working directory. Used to scope history search to
    /// "this directory". Linux (`/proc/<pid>/cwd`) and macOS (`lsof`); `None`
    /// elsewhere or if the shell has exited.
    pub fn shell_cwd(&self) -> Option<String> {
        shell_cwd(self.backend.pty_id())
    }

    /// Re-check whether this tab is running anything as `root`. Cheap-ish (one
    /// `/proc` scan on Linux, one `ps` on macOS); the app calls this on a slow
    /// poll, not every frame.
    pub fn refresh_root_status(&mut self) {
        self.is_root = tab_runs_as_root(self.backend.pty_id());
    }
}

/// The shell to spawn when nothing is configured and `$SHELL` is unset.
#[cfg(windows)]
fn default_shell() -> String {
    log::warn!("no shell configured; falling back to powershell.exe");
    "powershell.exe".to_owned()
}

#[cfg(not(windows))]
fn default_shell() -> String {
    log::warn!("no shell configured and $SHELL is unset; falling back to /bin/bash");
    "/bin/bash".to_owned()
}

#[cfg(target_os = "linux")]
fn shell_cwd(shell_pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{shell_pid}/cwd"))
        .ok()?
        .to_str()
        .map(str::to_owned)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
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

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn has_child_process(_shell_pid: u32) -> bool {
    false
}

/// Whether `shell_pid` or any of its descendants is running with effective
/// uid 0. One `/proc` scan: read `ppid` from `stat` and the effective uid
/// from `status` for every process, then walk down from `shell_pid`.
#[cfg(target_os = "linux")]
fn tab_runs_as_root(shell_pid: u32) -> bool {
    use std::collections::HashMap;

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    // pid -> (ppid, is_root)
    let mut procs: HashMap<u32, (u32, bool)> = HashMap::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((ppid, _state)) = parse_stat_ppid_state(&stat) else {
            continue;
        };
        let is_root = std::fs::read_to_string(entry.path().join("status"))
            .ok()
            .and_then(|s| status_effective_uid(&s))
            .is_some_and(|uid| uid == 0);
        procs.insert(pid, (ppid, is_root));
    }

    // BFS down the process tree from the shell.
    let mut stack = vec![shell_pid];
    let mut seen = vec![shell_pid];
    while let Some(pid) = stack.pop() {
        if procs.get(&pid).is_some_and(|&(_, root)| root) {
            return true;
        }
        for (&child, &(ppid, _)) in &procs {
            if ppid == pid && !seen.contains(&child) {
                seen.push(child);
                stack.push(child);
            }
        }
    }
    false
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn tab_runs_as_root(_shell_pid: u32) -> bool {
    false
}

/// The effective uid (2nd field of the `Uid:` line) from `/proc/<pid>/status`.
#[cfg(target_os = "linux")]
fn status_effective_uid(status: &str) -> Option<u32> {
    let line = status.lines().find(|l| l.starts_with("Uid:"))?;
    line.split_whitespace().nth(2)?.parse().ok()
}

// ---- macOS: `ps` / `lsof` instead of `/proc` -------------------------------
//
// macOS has no `/proc`. `ps` and `lsof` are both in the base system, so these
// spawn a subprocess rather than take a crate dependency (`libproc` would pull
// `bindgen`). The functions are called on a slow poll / on demand, not per
// frame. Verified by the parser unit test + the macOS CI build; not yet run on
// real hardware.

/// One row of `ps -Ao pid=,ppid=,uid=,state=`. Kept target-agnostic so its
/// parser can be unit-tested everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PsRow {
    pid: u32,
    ppid: u32,
    /// The real uid — `sudo -s` / `su` / a spawned `sudo <cmd>` all end up
    /// with ruid 0, which is the intent of the Linux euid check.
    uid: u32,
    zombie: bool,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_ps_table(output: &str) -> Vec<PsRow> {
    output
        .lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            Some(PsRow {
                pid: f.next()?.parse().ok()?,
                ppid: f.next()?.parse().ok()?,
                uid: f.next()?.parse().ok()?,
                zombie: f.next().is_some_and(|s| s.starts_with('Z')),
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn mac_proc_table() -> Vec<PsRow> {
    std::process::Command::new("/bin/ps")
        .args(["-Ao", "pid=,ppid=,uid=,state="])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_ps_table(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn shell_cwd(shell_pid: u32) -> Option<String> {
    // `lsof -a -d cwd -p <pid> -Fn` prints `p<pid>` then `n<path>`, one per line.
    let out = std::process::Command::new("/usr/sbin/lsof")
        .args(["-a", "-d", "cwd", "-Fn", "-p", &shell_pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix('n').map(str::to_owned))
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "macos")]
fn has_child_process(shell_pid: u32) -> bool {
    mac_proc_table()
        .iter()
        .any(|p| p.ppid == shell_pid && !p.zombie)
}

#[cfg(target_os = "macos")]
fn tab_runs_as_root(shell_pid: u32) -> bool {
    let table = mac_proc_table();
    let mut stack = vec![shell_pid];
    let mut seen = vec![shell_pid];
    while let Some(pid) = stack.pop() {
        if table.iter().any(|p| p.pid == pid && p.uid == 0) {
            return true;
        }
        for p in &table {
            if p.ppid == pid && !seen.contains(&p.pid) {
                seen.push(p.pid);
                stack.push(p.pid);
            }
        }
    }
    false
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{parse_stat_ppid_state, status_effective_uid};

    #[test]
    fn reads_the_effective_uid_from_status() {
        let status = "Name:\tbash\nState:\tS (sleeping)\nUid:\t1000\t1000\t1000\t1000\n";
        assert_eq!(status_effective_uid(status), Some(1000));
        let root = "Name:\tbash\nUid:\t0\t0\t0\t0\nGid:\t0\t0\t0\t0\n";
        assert_eq!(status_effective_uid(root), Some(0));
        assert_eq!(status_effective_uid("no uid line here"), None);
    }

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
mod ps_table_tests {
    use super::{PsRow, parse_ps_table};

    #[test]
    fn parses_a_ps_table() {
        // `ps -Ao pid=,ppid=,uid=,state=` output (leading spaces, varied cols).
        let out = "    1     0     0 Ss\n  842   700   501 S\n  900   842     0 R+\n  931   900   501 Z\ngarbage line\n";
        let rows = parse_ps_table(out);
        assert_eq!(
            rows,
            vec![
                PsRow {
                    pid: 1,
                    ppid: 0,
                    uid: 0,
                    zombie: false
                },
                PsRow {
                    pid: 842,
                    ppid: 700,
                    uid: 501,
                    zombie: false
                },
                PsRow {
                    pid: 900,
                    ppid: 842,
                    uid: 0,
                    zombie: false
                },
                PsRow {
                    pid: 931,
                    ppid: 900,
                    uid: 501,
                    zombie: true
                },
            ]
        );
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
