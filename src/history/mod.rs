//! Command history.
//!
//! Milestone 3 will record shell commands (via OSC 133 shell integration) into a
//! SQLite database and expose a `Ctrl+R` fuzzy search over them. For now this is
//! a no-op sink so the rest of the app can already call into it.
#![allow(dead_code)] // scaffolding for the history milestone

/// One recorded command invocation.
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
}

/// Where recorded commands go. The milestone-1 implementation drops them.
pub trait HistoryStore: Send {
    fn record(&mut self, record: CommandRecord);
    fn search(&self, query: &str, limit: usize) -> Vec<CommandRecord>;
}

/// A store that keeps nothing.
#[derive(Default)]
pub struct NullHistory;

impl HistoryStore for NullHistory {
    fn record(&mut self, _record: CommandRecord) {}
    fn search(&self, _query: &str, _limit: usize) -> Vec<CommandRecord> {
        Vec::new()
    }
}
