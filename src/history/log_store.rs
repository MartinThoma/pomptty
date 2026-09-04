//! Reads the shell-hook history logs back for the `Ctrl+R` search overlay.
//!
//! The hook (see [`super::integration`]) appends one line per command to
//! `$POMPTTY_HISTORY_DIR/<pid>.log`. [`LogStore`] loads every `*.log` in that
//! directory; [`LogStore::query`] collapses repeated command lines into one row
//! each and ranks them for a search box — fuzzy relevance first, then recency
//! and frequency.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

use super::CommandRecord;

/// One command in the search results: every run of an identical command line
/// collapsed into a single row.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub command: String,
    /// Working directory of the most recent run.
    pub cwd: Option<String>,
    /// Exit status of the most recent run.
    pub exit_code: Option<i32>,
    /// When the command last started running.
    pub last_run: SystemTime,
    /// How many times it has been run.
    pub count: u32,
}

/// In-memory view of the history logs. Cheap to construct; [`refresh`] loads
/// (or reloads) the files, and only does the work when the directory changed.
///
/// [`refresh`]: LogStore::refresh
pub struct LogStore {
    dir: Option<PathBuf>,
    runs: Vec<CommandRecord>,
    /// Directory mtime at the last load, so `refresh` can skip a re-read.
    loaded_mtime: Option<SystemTime>,
    loaded: bool,
}

impl LogStore {
    pub fn new() -> Self {
        Self::at(super::history_dir())
    }

    fn at(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            runs: Vec::new(),
            loaded_mtime: None,
            loaded: false,
        }
    }

    /// Reload the logs if the history directory changed since the last load (or
    /// was never loaded). Called when the overlay opens.
    pub fn refresh(&mut self) {
        let Some(dir) = self.dir.clone() else { return };
        let mtime = std::fs::metadata(&dir).and_then(|m| m.modified()).ok();
        if self.loaded && mtime == self.loaded_mtime {
            return;
        }
        self.loaded = true;
        self.loaded_mtime = mtime;
        self.runs = read_dir(&dir);
    }

    /// Whether no history has been recorded yet (the shell hook isn't installed,
    /// or nothing has been run under it).
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Ranked search results for `query`. An empty query returns the most
    /// recently run commands; otherwise commands are fuzzy-matched against their
    /// text and ranked by relevance, then recency and frequency. `only_cwd`
    /// restricts to runs from that exact directory.
    pub fn query(&self, query: &str, only_cwd: Option<&str>, limit: usize) -> Vec<Hit> {
        let now = SystemTime::now();
        let mut hits = collapse(&self.runs, only_cwd);

        let query = query.trim();
        if query.is_empty() {
            hits.sort_by_key(|h| std::cmp::Reverse(h.last_run));
            hits.truncate(limit);
            return hits;
        }

        let index: HashMap<&str, usize> = hits
            .iter()
            .enumerate()
            .map(|(i, h)| (h.command.as_str(), i))
            .collect();
        let commands: Vec<&str> = hits.iter().map(|h| h.command.as_str()).collect();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

        let mut scored: Vec<(usize, f64)> = pattern
            .match_list(commands, &mut matcher)
            .into_iter()
            .filter_map(|(cmd, fuzzy)| {
                let i = *index.get(cmd)?;
                let h = &hits[i];
                let score = fuzzy as f64 + recency_bonus(h.last_run, now) + freq_bonus(h.count);
                Some((i, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit);
        scored.into_iter().map(|(i, _)| hits[i].clone()).collect()
    }

    /// Distinct directories commands have run in, most-recently-used first.
    /// Feeds the omnibox's "cd to a recent directory" section.
    pub fn recent_dirs(&self, limit: usize) -> Vec<String> {
        let mut runs: Vec<&CommandRecord> = self.runs.iter().collect();
        runs.sort_by_key(|r| std::cmp::Reverse(r.last_run));
        let mut seen = std::collections::HashSet::new();
        runs.into_iter()
            .filter_map(|r| r.cwd.clone())
            .filter(|d| seen.insert(d.clone()))
            .take(limit)
            .collect()
    }
}

impl Default for LogStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Collapse the raw run list into one [`Hit`] per distinct command line, keeping
/// the most recent run's cwd / exit and a total run count. Insertion order is
/// preserved so an empty-query sort is stable.
fn collapse(runs: &[CommandRecord], only_cwd: Option<&str>) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for r in runs {
        if let Some(want) = only_cwd
            && r.cwd.as_deref() != Some(want)
        {
            continue;
        }
        match index.get(&r.command).copied() {
            Some(i) => {
                let h = &mut hits[i];
                h.count += 1;
                if r.last_run >= h.last_run {
                    h.last_run = r.last_run;
                    h.exit_code = r.exit_code;
                    h.cwd = r.cwd.clone();
                }
            }
            None => {
                index.insert(r.command.clone(), hits.len());
                hits.push(Hit {
                    command: r.command.clone(),
                    cwd: r.cwd.clone(),
                    exit_code: r.exit_code,
                    last_run: r.last_run,
                    count: 1,
                });
            }
        }
    }
    hits
}

fn read_dir(dir: &Path) -> Vec<CommandRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut runs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // The hook also drops `.pending.<pid>` handoff files in here.
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        runs.extend(text.lines().filter_map(CommandRecord::parse_line));
    }
    runs
}

/// Tie-breaking bonus for a recent command: ~40 for one just run, decaying with
/// age (~28 after a day, ~14 after a week). Small next to a fuzzy match score so
/// relevance stays dominant.
fn recency_bonus(last_run: SystemTime, now: SystemTime) -> f64 {
    let age = now
        .duration_since(last_run)
        .unwrap_or_default()
        .as_secs_f64();
    let day = 86_400.0;
    40.0 * (day / (age + day)).sqrt()
}

/// Tie-breaking bonus for a frequently-run command: `ln(count) * 8`.
fn freq_bonus(count: u32) -> f64 {
    (count.max(1) as f64).ln() * 8.0
}

/// A compact "how long ago" label: `now`, `30s`, `5m`, `2h`, `4d`, `6w`, `2mo`,
/// `1y`.
pub fn humanize_since(t: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(t).unwrap_or_default().as_secs();
    match secs {
        0..=4 => "now".to_string(),
        5..=59 => format!("{secs}s"),
        60..=3_599 => format!("{}m", secs / 60),
        3_600..=86_399 => format!("{}h", secs / 3_600),
        86_400..=604_799 => format!("{}d", secs / 86_400),
        604_800..=2_591_999 => format!("{}w", secs / 604_800),
        2_592_000..=31_535_999 => format!("{}mo", secs / 2_592_000),
        _ => format!("{}y", secs / 31_536_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn write_log(dir: &Path, pid: u32, records: &[CommandRecord]) {
        let mut body = String::new();
        for r in records {
            body.push_str(&r.to_line());
            body.push('\n');
        }
        std::fs::write(dir.join(format!("{pid}.log")), body).unwrap();
    }

    fn rec(command: &str, cwd: Option<&str>, ago: Duration) -> CommandRecord {
        CommandRecord {
            command: command.to_owned(),
            cwd: cwd.map(str::to_owned),
            exit_code: Some(0),
            last_run: SystemTime::now() - ago,
            duration: None,
        }
    }

    fn store_over(records: &[CommandRecord]) -> (tempfile::TempDir, LogStore) {
        let dir = tempfile::tempdir().unwrap();
        write_log(dir.path(), 1, records);
        let mut store = LogStore::at(Some(dir.path().to_path_buf()));
        store.refresh();
        (dir, store)
    }

    #[test]
    fn collapses_repeats_keeping_the_newest_run() {
        let (_d, store) = store_over(&[
            rec("make", Some("/a"), Duration::from_secs(600)),
            rec("ls", None, Duration::from_secs(300)),
            rec("make", Some("/b"), Duration::from_secs(120)),
        ]);
        let hits = store.query("", None, 10);
        assert_eq!(hits.len(), 2, "repeats collapse into one row");
        assert_eq!(hits[0].command, "make", "most recent run sorts first");
        assert_eq!(hits[1].command, "ls");
        let make = &hits[0];
        assert_eq!(make.count, 2);
        assert_eq!(make.cwd.as_deref(), Some("/b"), "newest run's cwd wins");
    }

    #[test]
    fn empty_query_orders_by_recency_across_files() {
        let dir = tempfile::tempdir().unwrap();
        write_log(dir.path(), 1, &[rec("old", None, Duration::from_secs(900))]);
        write_log(dir.path(), 2, &[rec("new", None, Duration::from_secs(10))]);
        let mut store = LogStore::at(Some(dir.path().to_path_buf()));
        store.refresh();
        let hits = store.query("", None, 10);
        assert_eq!(
            hits.iter().map(|h| h.command.as_str()).collect::<Vec<_>>(),
            ["new", "old"]
        );
    }

    #[test]
    fn fuzzy_query_narrows_and_ranks() {
        let (_d, store) = store_over(&[
            rec("git commit -m wip", None, Duration::from_secs(50)),
            rec("git checkout main", None, Duration::from_secs(40)),
            rec("cargo build", None, Duration::from_secs(30)),
        ]);
        let hits = store.query("gitco", None, 10);
        assert_eq!(hits.len(), 2, "only the git commands match");
        assert!(hits.iter().all(|h| h.command.starts_with("git")));
    }

    #[test]
    fn cwd_filter_restricts_results() {
        let (_d, store) = store_over(&[
            rec("ls", Some("/home/a"), Duration::from_secs(50)),
            rec("pwd", Some("/home/b"), Duration::from_secs(40)),
        ]);
        let hits = store.query("", Some("/home/a"), 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].command, "ls");
    }

    #[test]
    fn query_respects_the_limit() {
        let recs: Vec<_> = (0..10u64)
            .map(|i| rec(&format!("cmd{i}"), None, Duration::from_secs(i * 10 + 10)))
            .collect();
        let (_d, store) = store_over(&recs);
        assert_eq!(store.query("", None, 3).len(), 3);
        assert_eq!(store.query("cmd", None, 3).len(), 3);
    }

    #[test]
    fn recent_dirs_are_deduped_and_most_recent_first() {
        let (_d, store) = store_over(&[
            rec("ls", Some("/home/a"), Duration::from_secs(500)),
            rec("pwd", Some("/home/b"), Duration::from_secs(300)),
            rec("ls", Some("/home/a"), Duration::from_secs(100)), // /home/a again, more recently
            rec("true", None, Duration::from_secs(50)),           // no cwd recorded
        ]);
        assert_eq!(store.recent_dirs(10), ["/home/a", "/home/b"]);
        assert_eq!(store.recent_dirs(1), ["/home/a"], "respects the limit");
    }

    #[test]
    fn missing_directory_is_empty_not_an_error() {
        let mut store = LogStore::at(Some(PathBuf::from("/no/such/pomptty/history")));
        store.refresh();
        assert!(store.is_empty());
        assert!(store.query("anything", None, 10).is_empty());
    }

    #[test]
    fn humanize_since_buckets() {
        let now = SystemTime::now();
        let ago = |s| now - Duration::from_secs(s);
        assert_eq!(humanize_since(now, now), "now");
        assert_eq!(humanize_since(ago(30), now), "30s");
        assert_eq!(humanize_since(ago(300), now), "5m");
        assert_eq!(humanize_since(ago(7_200), now), "2h");
        assert_eq!(humanize_since(ago(2 * 86_400), now), "2d");
        assert_eq!(humanize_since(ago(14 * 86_400), now), "2w");
        assert_eq!(humanize_since(ago(60 * 86_400), now), "2mo");
        assert_eq!(humanize_since(ago(400 * 86_400), now), "1y");
    }
}
