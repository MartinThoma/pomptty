//! Parsing and formatting of a single history log line.
//!
//! See the module docs in [`super`] for the field layout.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;

use super::CommandRecord;

const SENTINEL: &str = "-";

impl CommandRecord {
    /// Parse one log line. Returns `None` for a blank, malformed, or
    /// partially-written line (the reader simply skips those).
    pub fn parse_line(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\n', '\r']);
        if line.is_empty() {
            return None;
        }

        let mut fields = line.split('\t');
        let start = fields.next()?;
        let duration = fields.next()?;
        let exit = fields.next()?;
        let cwd_b64 = fields.next()?;
        let command_b64 = fields.next()?;
        if fields.next().is_some() {
            return None; // more fields than expected
        }

        let last_run = parse_epoch(start)?;
        let duration = match duration {
            SENTINEL => None,
            s => Some(parse_secs(s)?),
        };
        let exit_code = match exit {
            SENTINEL => None,
            s => Some(s.parse().ok()?),
        };
        let cwd = decode(cwd_b64)?;
        let cwd = Some(cwd).filter(|s| !s.is_empty());
        let command = decode(command_b64)?;
        if command.is_empty() {
            return None;
        }

        Some(Self {
            command,
            cwd,
            exit_code,
            last_run,
            duration,
        })
    }

    /// Render this record as a log line (without the trailing newline). Used by
    /// tests and a future log compactor; the shell hook writes the same shape.
    pub fn to_line(&self) -> String {
        let start = self
            .last_run
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let duration = match self.duration {
            Some(d) => format!("{:.6}", d.as_secs_f64()),
            None => SENTINEL.to_owned(),
        };
        let exit = match self.exit_code {
            Some(c) => c.to_string(),
            None => SENTINEL.to_owned(),
        };
        format!(
            "{start:.6}\t{duration}\t{exit}\t{}\t{}",
            B64.encode(self.cwd.as_deref().unwrap_or("")),
            B64.encode(&self.command),
        )
    }
}

fn parse_epoch(s: &str) -> Option<SystemTime> {
    let secs = parse_secs(s)?;
    UNIX_EPOCH.checked_add(secs)
}

/// Parse a non-negative, finite float number of seconds.
fn parse_secs(s: &str) -> Option<Duration> {
    let secs: f64 = s.parse().ok()?;
    Duration::try_from_secs_f64(secs).ok()
}

/// Decode a base64 field to a `String`. `None` = not valid base64 or not UTF-8,
/// in which case the whole line is skipped.
fn decode(b64: &str) -> Option<String> {
    String::from_utf8(B64.decode(b64).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(command: &str, cwd: Option<&str>) -> CommandRecord {
        CommandRecord {
            command: command.to_owned(),
            cwd: cwd.map(str::to_owned),
            exit_code: Some(0),
            last_run: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            duration: Some(Duration::from_millis(1500)),
        }
    }

    #[test]
    fn round_trips() {
        let r = record("cargo build --release", Some("/home/moose/pomptty"));
        assert_eq!(CommandRecord::parse_line(&r.to_line()), Some(r));
    }

    #[test]
    fn round_trips_awkward_commands() {
        for cmd in [
            "echo 'a\tb'",
            "printf 'line1\nline2\n'",
            "grep -R \"needle\" .",
            "python -c 'print(\"日本語\")'",
        ] {
            let r = record(cmd, Some("/tmp/x y"));
            assert_eq!(
                CommandRecord::parse_line(&r.to_line()).as_ref(),
                Some(&r),
                "failed for {cmd:?}",
            );
        }
    }

    #[test]
    fn sentinels_become_none() {
        let r = CommandRecord::parse_line(&format!(
            "1700000000.000000\t-\t-\t{}\t{}",
            B64.encode(""),
            B64.encode("make"),
        ))
        .unwrap();
        assert_eq!(r.duration, None);
        assert_eq!(r.exit_code, None);
        assert_eq!(r.cwd, None);
        assert_eq!(r.command, "make");
    }

    #[test]
    fn rejects_malformed() {
        for line in [
            "",
            "   ",
            "not a record",
            "1700000000\tonly\tthree",
            // 6 fields
            &format!("1\t1\t0\t{}\t{}\textra", B64.encode("/x"), B64.encode("ls")),
            // bad base64 command
            "1700000000.0\t1.0\t0\tL3g=\t!!!notb64!!!",
            // empty command
            &format!(
                "1700000000.0\t1.0\t0\t{}\t{}",
                B64.encode("/x"),
                B64.encode("")
            ),
            // negative start
            &format!("-5\t1.0\t0\t{}\t{}", B64.encode("/x"), B64.encode("ls")),
        ] {
            assert_eq!(
                CommandRecord::parse_line(line),
                None,
                "should reject {line:?}"
            );
        }
    }

    #[test]
    fn tolerates_trailing_newline() {
        let r = record("ls", None);
        assert_eq!(
            CommandRecord::parse_line(&format!("{}\n", r.to_line())),
            Some(r),
        );
    }
}
