//! Session restore: remember the open tabs (directory + any rename) so the
//! next launch — after a normal quit or a crash — can reopen them.
//!
//! Saved periodically and once more on a clean shutdown (see
//! `PompttyApp::{maybe_persist_session, on_exit}`), to a small JSON file next
//! to the history logs. This is best-effort background state, not something a
//! user hand-edits: unlike `config.json`, a missing or malformed file is never
//! an error — [`Session::load`] just returns `None` and pomptty falls back to
//! a single default tab.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::terminal::{TabColor, TerminalTab};

/// One remembered tab.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedTab {
    pub cwd: Option<String>,
    /// A rename override, if the tab had one. The shell's own OSC title is
    /// not saved — it's transient and the restored shell will set its own.
    pub title: Option<String>,
    pub color: Option<TabColor>,
}

/// The whole remembered session.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Session {
    pub tabs: Vec<SavedTab>,
    pub active: usize,
}

/// `~/.local/state/pomptty/session.json` on Linux — [`history::history_dir`]'s
/// sibling.
///
/// [`history::history_dir`]: crate::history::history_dir
pub fn session_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "pomptty")?;
    let base = dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .to_path_buf();
    Some(base.join("session.json"))
}

impl Session {
    /// Snapshot the open tabs.
    pub fn capture(tabs: &[TerminalTab], active: usize) -> Self {
        Self {
            tabs: tabs
                .iter()
                .map(|t| SavedTab {
                    cwd: t.shell_cwd(),
                    title: t.manual_title.clone(),
                    color: t.color,
                })
                .collect(),
            active,
        }
    }

    /// Load the last saved session. `None` if there isn't one, or it can't be
    /// read/parsed — logged, never fatal, so a corrupt file can't block
    /// startup.
    pub fn load() -> Option<Self> {
        Self::load_from(&session_path()?)
    }

    fn load_from(path: &Path) -> Option<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("could not read {}: {e:#}", path.display());
                return None;
            }
        };
        match serde_json::from_str(&text) {
            Ok(session) => Some(session),
            Err(e) => {
                log::warn!("ignoring unreadable session file {}: {e:#}", path.display());
                None
            }
        }
    }

    /// Write this session to disk, creating the parent directory as needed.
    pub fn save(&self) -> Result<()> {
        self.save_to(&session_path().context("could not determine a session directory")?)
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
    }
}

/// Where the active tab lands after restoring a possibly-shorter tab list
/// than what was saved (a tab that failed to spawn, say). Empty list -> `0`.
pub fn restore_active(saved_active: usize, tab_count: usize) -> usize {
    if tab_count == 0 {
        0
    } else {
        saved_active.min(tab_count - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_active_clamps_to_the_tab_count() {
        assert_eq!(restore_active(1, 3), 1, "in range, unchanged");
        assert_eq!(
            restore_active(5, 3),
            2,
            "past the end clamps to the last tab"
        );
        assert_eq!(restore_active(0, 0), 0, "no tabs at all");
    }

    fn sample() -> Session {
        Session {
            tabs: vec![
                SavedTab {
                    cwd: Some("/tmp".to_owned()),
                    title: Some("build".to_owned()),
                    color: Some(TabColor::Blue),
                },
                SavedTab {
                    cwd: None,
                    title: None,
                    color: None,
                },
            ],
            active: 1,
        }
    }

    #[test]
    fn saves_and_loads_back_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let session = sample();

        session.save_to(&path).unwrap();
        assert_eq!(Session::load_from(&path), Some(session));
    }

    #[test]
    fn old_session_file_without_color_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        std::fs::write(
            &path,
            r#"{ "tabs": [ { "cwd": "/tmp", "title": null } ], "active": 0 }"#,
        )
        .unwrap();

        let loaded = Session::load_from(&path).expect("a pre-color session.json still parses");
        assert_eq!(loaded.tabs[0].color, None);
    }

    #[test]
    fn missing_file_is_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("session.json");
        assert_eq!(Session::load_from(&path), None);
    }

    #[test]
    fn malformed_file_is_ignored_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Session::load_from(&path), None);
    }

    // `Session::capture` needs real `TerminalTab`s (a spawned PTY), which unit
    // tests avoid — its two-line body (map cwd + manual_title) is exercised
    // end-to-end instead (see the plan's Verification section).
}
