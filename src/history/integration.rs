//! Shell-integration snippets printed by `pomptty --print-integration <shell>`.
//!
//! Each snippet is a no-op unless `$POMPTTY` and `$POMPTTY_HISTORY_DIR` are set
//! (pomptty exports both), so it is safe to add to a shell rc unconditionally.
//! It appends one [`super::record`]-format line per command to
//! `$POMPTTY_HISTORY_DIR/<shell-pid>.log`.
//!
//! Timestamps use `date +%s.%N` and durations are computed with `LC_ALL=C awk`,
//! so the decimal point is always `.` regardless of the user's locale.

/// The snippet for `shell` (`bash` / `zsh` / `fish`), or `None` if unsupported.
pub fn snippet(shell: &str) -> Option<&'static str> {
    match shell {
        "bash" => Some(BASH),
        "zsh" => Some(ZSH),
        "fish" => Some(FISH),
        _ => None,
    }
}

/// Shells `snippet` knows about, for help text / error messages.
pub const SUPPORTED: &[&str] = &["bash", "zsh", "fish"];

const BASH: &str = r#"# pomptty shell integration (bash) -- inert outside pomptty.
if [ -n "${POMPTTY:-}" ] && [ -n "${POMPTTY_HISTORY_DIR:-}" ] && [ -n "${BASH_VERSION:-}" ]; then
  __pomptty_b64() { printf '%s' "$1" | base64 | tr -d '\n'; }
  __pomptty_dur() { LC_ALL=C awk -v a="$1" -v b="$2" 'BEGIN { d = b - a; printf "%.6f", (d < 0 ? 0 : d) }'; }
  # Our own stale pending file (crash / PID reuse) plus any left by shells that
  # exited before their prompt hook ran.
  rm -f "$POMPTTY_HISTORY_DIR/.pending.$$" 2>/dev/null
  find "$POMPTTY_HISTORY_DIR" -maxdepth 1 -name '.pending.*' -mmin +60 -delete 2>/dev/null

  # Runs (via PS0) once, right after a command line is read.
  __pomptty_preexec() {
    local n c
    { read -r n c; } <<< "$(HISTTIMEFORMAT= builtin history 1)"
    [ -n "$c" ] || return 0
    [ "$n" = "${__pomptty_last_n:-}" ] && return 0   # bare Enter: no new command
    printf '%s\t%s\t%s\n' "$n" "$(date +%s.%N)" "$(__pomptty_b64 "$c")" \
      > "$POMPTTY_HISTORY_DIR/.pending.$$" 2>/dev/null
  }

  # Runs (via PROMPT_COMMAND) once, before the next prompt is drawn. Leaves $?
  # untouched for anything chained after it in PROMPT_COMMAND.
  __pomptty_precmd() {
    local code=$? pend="$POMPTTY_HISTORY_DIR/.pending.$$"
    [ -f "$pend" ] || return $code
    local n start cmd_b64
    { IFS=$'\t' read -r n start cmd_b64; } < "$pend"
    rm -f "$pend"
    __pomptty_last_n=$n
    if [ -n "$cmd_b64" ]; then
      printf '%s\t%s\t%s\t%s\t%s\n' \
        "$start" "$(__pomptty_dur "$start" "$(date +%s.%N)")" "$code" \
        "$(__pomptty_b64 "$PWD")" "$cmd_b64" \
        >> "$POMPTTY_HISTORY_DIR/$$.log" 2>/dev/null
    fi
    return $code
  }

  case "${PROMPT_COMMAND:-}" in
    *__pomptty_precmd*) ;;
    *) PROMPT_COMMAND="__pomptty_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}" ;;
  esac
  case "${PS0:-}" in
    *__pomptty_preexec*) ;;
    *) PS0='$(__pomptty_preexec)'"${PS0:-}" ;;
  esac
fi
"#;

const ZSH: &str = r#"# pomptty shell integration (zsh) -- inert outside pomptty.
if [[ -n ${POMPTTY:-} && -n ${POMPTTY_HISTORY_DIR:-} ]]; then
  typeset -g __pomptty_start __pomptty_cmd

  __pomptty_b64() { print -rn -- "$1" | base64 | tr -d '\n' }
  __pomptty_dur() { LC_ALL=C awk -v a="$1" -v b="$2" 'BEGIN { d = b - a; printf "%.6f", (d < 0 ? 0 : d) }' }

  __pomptty_preexec() {
    __pomptty_start=$(date +%s.%N)
    __pomptty_cmd=$1
  }

  __pomptty_precmd() {
    local code=$?
    if [[ -n $__pomptty_cmd ]]; then
      printf '%s\t%s\t%s\t%s\t%s\n' \
        "$__pomptty_start" "$(__pomptty_dur "$__pomptty_start" "$(date +%s.%N)")" "$code" \
        "$(__pomptty_b64 "$PWD")" "$(__pomptty_b64 "$__pomptty_cmd")" \
        >> "$POMPTTY_HISTORY_DIR/$$.log" 2>/dev/null
      __pomptty_cmd=
    fi
    return $code
  }

  autoload -Uz add-zsh-hook
  add-zsh-hook preexec __pomptty_preexec
  add-zsh-hook precmd __pomptty_precmd
fi
"#;

const FISH: &str = r#"# pomptty shell integration (fish) -- inert outside pomptty.
if set -q POMPTTY; and set -q POMPTTY_HISTORY_DIR
  function __pomptty_b64
    printf '%s' "$argv[1]" | base64 | tr -d '\n'
  end

  function __pomptty_preexec --on-event fish_preexec
    set -g __pomptty_start (date +%s.%N)
    set -g __pomptty_cmd $argv[1]
  end

  function __pomptty_postexec --on-event fish_postexec
    set -l code $status
    if set -q __pomptty_cmd; and test -n "$__pomptty_cmd"
      set -l dur (env LC_ALL=C awk -v a=$__pomptty_start -v b=(date +%s.%N) 'BEGIN { d = b - a; printf "%.6f", (d < 0 ? 0 : d) }')
      printf '%s\t%s\t%s\t%s\t%s\n' \
        "$__pomptty_start" "$dur" "$code" \
        (__pomptty_b64 "$PWD") (__pomptty_b64 "$__pomptty_cmd") \
        >> "$POMPTTY_HISTORY_DIR/$fish_pid.log" 2>/dev/null
      set -e __pomptty_cmd
    end
    return $code
  end
end
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_shells_resolve() {
        for s in SUPPORTED {
            assert!(snippet(s).is_some(), "no snippet for {s}");
        }
        assert!(snippet("tcsh").is_none());
        assert!(snippet("").is_none());
    }

    #[test]
    fn snippets_are_guarded_and_write_the_log() {
        for s in SUPPORTED {
            let body = snippet(s).unwrap();
            assert!(body.contains("POMPTTY_HISTORY_DIR"), "{s}: not guarded");
            assert!(body.contains(".log"), "{s}: never writes the log");
            assert!(body.contains("LC_ALL=C"), "{s}: locale-unsafe arithmetic");
        }
    }
}
