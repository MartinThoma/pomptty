# Security

## Reporting a vulnerability

Please report security issues **privately** through GitHub's
["Report a vulnerability"](https://github.com/MartinThoma/pomptty/security/advisories/new)
button (Security → Advisories). Don't open a public issue for anything
exploitable.

You should get an acknowledgement within a week. Once a fix is ready we'll
coordinate a disclosure date with you and credit you in the release notes
unless you'd rather stay anonymous.

## What pomptty does — and doesn't

Everything below is meant to be checkable, not just asserted.

### No network

pomptty opens **no network connections**. There is no HTTP client, no
telemetry, no analytics, no crash reporting, and no auto-update anywhere in
the dependency tree.

```sh
# no HTTP client crate:
grep -E '^name = "(reqwest|hyper|ureq|isahc|curl|surf|attohttpc)"' Cargo.lock   # → nothing
# no sockets at runtime:
ss -tupn | grep pomptty                                                          # → nothing
```

The only outward-facing actions are all local or explicitly user-initiated:

| Action | When | How |
|---|---|---|
| Open a URL in your browser | you `Ctrl`+click an OSC 8 hyperlink | `open` crate → `xdg-open` |
| Post a desktop notification | a long command finishes while unfocused | local D-Bus (`notify-rust`) |
| Read/write the clipboard | copy / paste | local X11 / Wayland (`arboard`) |

### Runs as you, unprivileged

pomptty never asks for or uses elevated privileges. It is not setuid, spawns
no privileged helper, and needs no capabilities. It runs your `$SHELL` as a
child process with your uid.

### Files it touches

| Path | Access |
|---|---|
| `$XDG_CONFIG_HOME/pomptty/config.json` | read; created with defaults on first run |
| `$XDG_STATE_HOME/pomptty/history/*.log` | read (for `Ctrl+R`); written **only** by the shell hook |
| `$XDG_STATE_HOME/pomptty/session.json` | read + written (tab restore) |
| the path in `font_family`, if you set one | read |
| `/proc/<shell-pid>/{cwd,stat,status}` (Linux) | read (cwd, "still running?", root check) |

Nothing else is read or written.

### The shell-integration hook

Command history is recorded by a shell snippet you add yourself. Run
`pomptty --print-integration bash` (or `zsh`/`fish`) and read it before
pasting it into your rc. It is inert unless `$POMPTTY` and
`$POMPTTY_HISTORY_DIR` are both set (pomptty sets them), and it only ever
*appends* one line per command to `$POMPTTY_HISTORY_DIR`.

### Vendored dependency

`vendor/egui_term/` is a fork of
[`Harzu/egui_term`](https://github.com/Harzu/egui_term) pinned at a specific
revision (see `vendor/egui_term/src/lib.rs`). It's checked in, so every
change from upstream is reviewable as an ordinary diff — there is no opaque
git or binary dependency.

## Auditing pomptty yourself

```sh
# every dependency: advisory-clean, permissively licensed, from crates.io
cargo install cargo-deny --locked && cargo deny check

# pomptty's own unsafe code — there is none
rg 'allow\(unsafe_code\)|unsafe ' src/

# the whole dependency graph
cargo tree
```

`cargo deny check` also runs on every push and pull request
(`.github/workflows/ci.yml`), and `src/main.rs` carries
`#![deny(unsafe_code)]` with no `#[allow(unsafe_code)]` escape hatches —
the shell's environment (`TERM`, `COLORTERM`, …) is handed to the child
process via `Command::env` (`terminal::shell_env`), never set on pomptty's
own process.

## Safety features

pomptty tries to make destructive mistakes harder:

- **Superuser warning** — when the shell (or `sudo -s` / `su` / a long
  `sudo …` under it) is running as `root`, the terminal gets a red outline
  and the tab a red dot. Off with `"security": { "superuser_warning": false }`.
- **Bracketed paste + a confirm dialog** — a multi-line paste into a shell
  that hasn't enabled bracketed paste asks first, instead of running each
  line on arrival.
- **Close confirmation** — closing a tab with a process still running asks
  first.
