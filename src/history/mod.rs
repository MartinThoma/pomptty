//! Command history.
//!
//! History is recorded by a **shell hook**, not by watching the terminal: the
//! escape-sequence route (OSC 133 / OSC 7) is a dead end because the parser
//! stack pomptty builds on (`vte` → `alacritty_terminal` → `egui_term`) surfaces
//! neither. `pomptty --print-integration <shell>` prints a snippet (see
//! [`integration`]) that appends one line per command to
//! `$POMPTTY_HISTORY_DIR/<shell-pid>.log`. `Ctrl+R` reads those files back.
//!
//! The log is append-only — one line per run. Duplicates are collapsed at query
//! time, which keeps a true "last executed" timestamp and per-run frequency data
//! rather than overwriting a single row per command.
//!
//! ## Line format
//!
//! Tab-separated, `\n`-terminated. `command` and `cwd` are base64 so the shell
//! never has to quote anything; see [`record`].
//!
//! ```text
//! <start_epoch>\t<duration_secs | "-">\t<exit_code | "-">\t<cwd_b64>\t<command_b64>
//! ```
#![allow(dead_code)] // parts are wired up across the M3 stages

pub mod integration;
pub mod record;

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// The directory the shell hook writes logs to and `Ctrl+R` reads them from —
/// `~/.local/state/pomptty/history` on Linux. Exported to shells as
/// `$POMPTTY_HISTORY_DIR`.
pub fn history_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "pomptty")?;
    let base = dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .to_path_buf();
    Some(base.join("history"))
}

/// One recorded command invocation.
///
/// As a single parsed log line this describes one run; as a search result the
/// duplicates have been collapsed and `last_run` / `duration` refer to the most
/// recent run.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRecord {
    pub command: String,
    pub cwd: Option<String>,
    /// Exit status. `None` if it was not recorded (see `duration`).
    pub exit_code: Option<i32>,
    /// Wall-clock time the command started running.
    pub last_run: SystemTime,
    /// How long the command ran. `None` when the shell hook never saw it start
    /// (a bare Enter, `Ctrl+C` at an empty prompt) or the run outlived the
    /// session.
    pub duration: Option<Duration>,
}
