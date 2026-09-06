# pomptty

[![CI](https://github.com/MartinThoma/pomptty/actions/workflows/ci.yml/badge.svg)](https://github.com/MartinThoma/pomptty/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A minimal, Chrome-flavored terminal emulator: GPU-rendered, tabbed, and driven
by a single JSON config file that reloads as you edit it.

![pomptty showing a shell session in a tab, rendered with the Solarized Dark theme](docs/screenshot.png)

pomptty is written in Rust on [`egui`](https://github.com/emilk/egui) +
[`egui_term`](https://github.com/Harzu/egui_term), which wraps
[`alacritty_terminal`](https://github.com/alacritty/alacritty) for VT parsing and
the PTY. Linux and Windows are supported; macOS is on the
[roadmap](ROADMAP.md). A few things are still Linux-only — see
[Known issues](ROADMAP.md#known-issues) in the roadmap.

## Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [Shell integration](#shell-integration)
- [Configuration](#configuration)
- [Development](#development)
- [Security](#security)
- [Roadmap](#roadmap)
- [License](#license)

## Features

- **Command palette** (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>) — one
  input, fuzzy-filtered across four ranked sections at once: actions, open
  tabs, command history, and recent directories. <kbd>Enter</kbd> acts on
  whatever's selected — run the action, switch tab, drop the command on the
  prompt, or `cd` the active tab there.
- **Tabs** — open, close, switch, and drag-to-reorder by keyboard or mouse; each
  tab shows the title set by the shell (OSC 0/2), the window title follows the
  active tab, and a new tab opens in the active tab's working directory.
  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd> reopens the last closed tab,
  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> opens a fuzzy tab switcher, and
  right-clicking a tab gives rename / duplicate / close-others / reopen-closed
  / color.
- **Session restore** — quitting (or crashing) and relaunching reopens the same
  tabs, in the same directories and order, with the same active tab and any
  renames. On by default; turn it off with `"session": { "restore": false }`.
- **Live configuration reload** — save `config.json` and the change applies
  immediately (or press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>R</kbd>). A
  malformed file is never overwritten: the last good config is kept and the error
  is shown.
- **Theming** — four builtins (Solarized Dark/Light, plain dark/light) or a full
  inline palette. The surrounding UI is tinted to match the terminal background.
  Apps can recolour the palette / fg / bg / cursor at runtime (OSC 4/10/11/12),
  so neovim colorschemes take effect.
- **Layered keybindings** — your bindings sit on top of the defaults, so new
  default shortcuts appear automatically and any default can be switched off.
- **Fuzzy history search** (<kbd>Ctrl</kbd>+<kbd>R</kbd>) — an overlay ranked by
  relevance, recency and frequency, showing each command's directory, exit
  status and age; opt-in via a [one-line shell hook](#shell-integration).
- **Long-command notifications** — a desktop notification when a command that
  ran 5+ minutes (configurable) finishes while pomptty is unfocused, minimised,
  or on another tab. Uses the same shell hook.
- Font zoom, scrollback keys, and clear-screen.
- Mouse selection with copy/paste
  (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd> /
  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>V</kbd>); double/triple-click select a
  word/line, <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+drag selects a column; select-to-copy
  and middle-click paste the X11 primary selection; bracketed paste so a
  multi-line paste doesn't auto-run; OSC 52 so `tmux` / `vim` can copy to the
  clipboard over SSH.
- Custom font from a file path, with real bold/italic faces when installed;
  underline (single / double / undercurl / dotted / dashed, with `\e[58m`
  underline colour) and strikethrough — neovim / helix LSP squiggles render.
- Box-drawing, block, shade and Powerline glyphs drawn by pomptty rather than
  the font — lines join with no sub-pixel gap and stay crisp at any size.
- Optional background image (dimmed, with a soft vignette) and, on
  Wayland / macOS / Windows, window translucency.
- A themed, shaped (block/beam/underline), gliding, blinking cursor — apps that
  set their own style (`vim`'s insert-mode beam) override the configured
  default.
- OSC 8 hyperlinks underline on plain hover; <kbd>Ctrl</kbd>+click opens the
  real target URI even when the visible text isn't a URL.
- A confirmation prompt before closing a tab that still has a process running.

## Installation

Tagged builds — a `.deb`, an `.rpm`, a Windows `.exe` and a macOS tarball —
are attached to each
[GitHub Release](https://github.com/MartinThoma/pomptty/releases). To build
from source instead:

### Prerequisites

- A current stable **Rust** toolchain (2024 edition). Install it with
  [rustup](https://rustup.rs/); a distro-packaged `cargo` is often too old.
- A working GPU stack at run time. On Linux: Vulkan or OpenGL, plus the X11 or
  Wayland client libraries — pomptty loads these dynamically, and a typical
  desktop already has them. On Windows: DirectX or Vulkan, already present on
  any current install — no extra packages needed either way.

pomptty asks for a **low-power** GPU (usually the integrated one) — a terminal
doesn't need a discrete card, and small dedicated GPUs can run out of VRAM.
The chosen adapter is printed at startup (`GPU: …`). To override: set
`WGPU_POWER_PREF=high` for the discrete GPU, or `WGPU_BACKEND=vulkan|gl|dx12`
to pin a backend.

### Build and run

```sh
git clone https://github.com/MartinThoma/pomptty
cd pomptty
cargo run --release
```

The first run writes a default `config.json` (see [Configuration](#configuration))
and starts your `$SHELL`.

### Make targets

The `Makefile` prepends `~/.cargo/bin` to `PATH`, which is handy when the system
`cargo` shadows the rustup one:

| Target | Action |
|---|---|
| `make run` | debug build, then run |
| `make build` | debug build |
| `make release` | optimized build |
| `make test` | run the test suite |
| `make lint` | `rustfmt --check` + `clippy -D warnings` |
| `make fmt` | format the source |
| `make windows` | cross-compile a release `pomptty.exe` from Linux — needs the `mingw-w64` linker (`sudo apt install mingw-w64` on Debian/Ubuntu) installed once; the `x86_64-pc-windows-gnu` rustup target is added automatically |
| `make deb` | build a `.deb` package (`target/debian/*.deb`); installs `cargo-deb` if missing — no sudo needed |
| `make rpm` | build an `.rpm` package (`target/generate-rpm/*.rpm`); installs `cargo-generate-rpm` if missing — no sudo needed |
| `make man` | preview the man page (`packaging/pomptty.1`) |
| `make icons` | re-rasterize the app icon from `packaging/pomptty.svg` |

Cutting a release is a tag push — see [RELEASING.md](RELEASING.md).

## Usage

### Keybindings

A binding maps a **chord** to an **action**. Chords are written like
`ctrl+shift+t`, `ctrl+plus`, or `shift+pageup`; the modifiers are `ctrl`,
`shift`, `alt`, and `super`.

Your `keybindings` block is **layered on the built-in defaults**: an entry
overrides or adds a chord, future default shortcuts appear without regenerating
the file, and binding a chord to `"disabled"` removes a default
(e.g. `"ctrl+r": "disabled"`).

| Action | Default | Notes |
|---|---|---|
| `copy` / `paste` | `ctrl+shift+c` / `ctrl+shift+v` | handled by the terminal widget |
| `font-increase` | `ctrl+plus`, `ctrl+equals` | |
| `font-decrease` | `ctrl+minus` | |
| `font-reset` | `ctrl+0` | |
| `scroll-page-up` / `scroll-page-down` | `shift+pageup` / `shift+pagedown` | |
| `scroll-to-top` / `scroll-to-bottom` | `shift+home` / `shift+end` | |
| `clear` | `ctrl+shift+k` | sends <kbd>Ctrl</kbd>+<kbd>L</kbd> to the shell |
| `reload-config` | `ctrl+shift+r` | |
| `new-tab` | — | not bound by default (`reopen-tab` covers `ctrl+shift+t`); bind it yourself for a guaranteed-new tab |
| `reopen-tab` | `ctrl+shift+t` | reopens the last closed tab (old slot, same directory); opens a plain new tab when there's nothing to reopen |
| `close-tab` | `ctrl+shift+w` | closing the last tab quits |
| `next-tab` / `prev-tab` | `ctrl+tab` / `ctrl+shift+tab` (also `ctrl+shift+pagedown` / `pageup`) | wraps around |
| `goto-tab-1` … `goto-tab-9` | `ctrl+1` … `ctrl+9` | jump to that tab; `goto-tab-9` is the last tab |
| `tab-search` | `ctrl+shift+a` | fuzzy switcher over the open tabs |
| `history-search` | `ctrl+r` | opens the fuzzy history overlay (see [Shell integration](#shell-integration)); falls through to the shell when `history.enabled` is `false` |
| `omnibox` | `ctrl+shift+p` | command palette: actions, tabs, history and recent directories in one ranked list |
| `window-maximize` / `window-restore` | — | also in the palette as "View: Maximize / Restore Window" |
| `window-left-half` / `window-right-half` | — | un-maximize and tile the window to that half of the screen; "View: Move to Left / Right Half" |
| `open-scrollback` | — | dump the active tab's scrollback to a temp file and open it in your default editor; "Terminal: Open Scrollback in Editor" |
| `disabled` | — | suppresses a default binding |

To send a raw byte string or escape sequence to the shell instead of running
an action, use the separate **`key_sends`** map — chord → string, where the
string understands `\e` / `\x1b`, `\xNN`, `\u{NNNN}`, `\n`, `\r`, `\t`, `\0`
and `\\`:

```json
"key_sends": { "alt+left": "b", "alt+right": "f", "ctrl+alt+k": "\u{1b}[1;5D" }
```

A chord listed in both `keybindings` and `key_sends` runs the action.

### Mouse

- Click a tab to focus it; click `×` or middle-click to close it; click `+` to
  open a new one.
- Drag a tab sideways to reorder it.
- Right-click a tab for rename, duplicate, close others, a "reopen closed"
  submenu, and a color (a stripe on the tab — 8 fixed swatches, or "None").
- Click and drag inside the terminal to select; selected text is available to
  copy.

### Closing a busy tab

Closing a tab that still has a child process running in it (an editor, `ssh`, a
build) asks for confirmation first. If that process exits while the prompt is
open, the tab closes as originally requested.

## Shell integration

pomptty records command history through a **shell hook** rather than by scraping
the terminal, so nothing is recorded until you opt in. Add one line to your
shell's rc file:

```sh
# ~/.bashrc  /  ~/.zshrc
eval "$(pomptty --print-integration bash)"   # or: zsh

# ~/.config/fish/config.fish
pomptty --print-integration fish | source
```

The hook only does anything inside pomptty (it checks for `$POMPTTY`), and it
leaves `$?` untouched. For each command it appends one line — start time,
duration, exit code, working directory, and the command — to
`$POMPTTY_HISTORY_DIR/<shell-pid>.log`
(`~/.local/state/pomptty/history/` by default).

Press <kbd>Ctrl</kbd>+<kbd>R</kbd> (or click the &#9906; in the tab strip) to
search it:

| Key | Action |
|---|---|
| type | fuzzy-filter; results rank by relevance, then recency and frequency |
| <kbd>↑</kbd> / <kbd>↓</kbd>, <kbd>Ctrl</kbd>+<kbd>P</kbd> / <kbd>Ctrl</kbd>+<kbd>N</kbd> | move the selection |
| <kbd>Tab</kbd> | toggle "this directory only" |
| <kbd>Enter</kbd> | put the command on the prompt |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | put it on the prompt and run it |
| <kbd>Esc</kbd> | close |

Set `"history": { "enabled": false }` to disable the overlay and let
<kbd>Ctrl</kbd>+<kbd>R</kbd> reach the shell's own reverse-i-search instead.

## Configuration

On first run, pomptty writes a default config to
`$XDG_CONFIG_HOME/pomptty/config.json` (usually `~/.config/pomptty/config.json`)
and generates it again if you delete it. Every field has a default, so a partial
file — even `{}` — is valid, and unknown fields are ignored.

[`config.example.json`](config.example.json) is a complete, commented-by-example
config.

| Field | Meaning |
|---|---|
| `font_family` | Path to a `.ttf`/`.otf`/`.ttc` file, **or** the name of an installed font family (case-insensitive). `null` uses the bundled monospace face. |
| `font_size` | Terminal font size, in points. Zooming with the keyboard writes the new size back here. |
| `bold_is_bright` | `false` by default. When `true`, bold text using one of the 8 normal palette colours is drawn with the matching bright colour (as Alacritty / kitty can). |
| `scrollback_lines` | Reserved; not yet wired to the backend. |
| `shell` | Shell to launch, or `null` for `$SHELL` (falling back to `/bin/bash`). |
| `shell_args` | Extra arguments for the shell. |
| `theme` | A builtin name or an inline palette object (see below). |
| `keybindings` | Map of chord → action (see [Keybindings](#keybindings)). |
| `key_sends` | Map of chord → raw byte string / escape sequence sent to the shell (see [Keybindings](#keybindings)). |
| `window` | `width` / `height` in logical pixels; `decorations`: `"custom"` (default) — frameless, pomptty's own tab strip is the title bar — or `"system"` to keep the OS title bar (use it if your WM handles a borderless window poorly); `opacity` (`0.05`–`1.0`, default `1.0`) — terminal-body translucency, **Wayland / macOS / Windows only** (ignored on X11); `background` — `{ "path": "…", "dim": 0.55, "vignette": 0.35 }` draws a PNG/JPEG behind the text. All take effect on restart. |
| `history` | `enabled` (default `true`) — whether <kbd>Ctrl</kbd>+<kbd>R</kbd> opens the overlay (see [Shell integration](#shell-integration)); `max_results` (default `50`) — rows shown at once. |
| `session` | `restore` (default `true`) — reopen the last run's tabs, directories and renames on launch; also stops pomptty recording them when `false`. Saved to `session.json` next to the history logs. |
| `cursor` | `shape` (default `"block"`; also `"beam"`, `"underline"`) and `blink` (default `true`) — the cursor's appearance before an app sets its own via DECSCUSR (`vim`'s insert-mode beam, for instance, still overrides this at runtime). |
| `paste` | `confirm_multiline` (default `true`) — ask before pasting newline-containing text into a shell that hasn't enabled bracketed paste, where each line would run on arrival. |
| `notifications` | `long_command_secs` (default `300`) — post a desktop notification when a command that ran at least this long finishes while pomptty is unfocused, minimised, or on another tab; `0` disables it. Needs the [shell integration](#shell-integration). |
| `clipboard` | `osc52_read` (default `false`) — let terminal apps *read* the system clipboard via OSC 52 (`\e]52;c;?`); the *copy* direction is always allowed. Off by default, matching Alacritty. New tabs pick up a change. |
| `security` | `superuser_warning` (default `true`) — red terminal outline + tab dot while the shell (or `sudo -s` / `su` / a long `sudo …` under it) is running as `root`. Linux and macOS. |

pomptty automatically adds an installed **Nerd Font / Powerline** font to the
fallback chain, so powerline prompts and devicon themes render their icons rather
than boxes — install one if you use such a prompt. Bold and italic text render
with the real bold/italic faces of the configured family when installed
(falling back to the regular face, never a synthetic embolden/oblique, when
they aren't). Ligatures aren't rendered yet (a terminal-backend limitation).

The terminal **bell** (`\a`) flashes the window briefly, and flags the taskbar
for attention if the window isn't focused.

### Theme

Set `theme` to a builtin name:

```json
"theme": "solarized-dark"
```

`"solarized-dark"` (default), `"solarized-light"`, `"default-dark"`, and
`"default-light"` are available.

Or provide an inline palette. The generated config does this with the full
Solarized Dark palette so every color is right there to edit:

```json
"theme": {
  "foreground": "#839496",
  "background": "#002b36",
  "cursor": "#93a1a1",
  "black": "#073642",
  "red": "#dc322f"
}
```

Recognized keys are `foreground`, `background`, `cursor`, and the 16 ANSI colors
(`black`…`white`, `bright_black`…`bright_white`). Any subset works; omitted colors
fall back to Solarized Dark.

## Development

```sh
make test      # cargo test
make lint      # rustfmt --check + clippy (warnings denied)
```

[GitHub Actions](.github/workflows/ci.yml) runs formatting, Clippy, the test
suite, and a release build on every push and pull request.

Source layout:

| Path | Contents |
|---|---|
| `src/app.rs` | the eframe app: tabs, event loop, dispatch |
| `src/config/` | config loading, themes, keybindings |
| `src/terminal/` | one PTY-backed terminal tab |
| `src/ui/` | the tab strip, history/tab-search/omnibox overlays, and other chrome |
| `src/history/` | the shell-integration hook, log format, and history store |
| `src/session.rs` | session restore: what's saved, load/save |

Contributions are welcome. Please run `make fmt lint test` before opening a pull
request.

## Security

pomptty is **local-only** — no network connections, no telemetry, no
auto-update — and runs entirely as your user with no elevated privileges. Its
own code is safe Rust bar two documented `env::set_var` blocks
(`#![deny(unsafe_code)]` enforces it), and every dependency is checked for
advisories, licenses and provenance by `cargo deny` on each push.

[SECURITY.md](SECURITY.md) spells out exactly what it does, which files it
touches, and how to verify all of it, plus how to report a vulnerability.

## Roadmap

See [ROADMAP.md](ROADMAP.md). Up next (M4/M5): a design pass on the chrome, and
command *blocks* — navigable, foldable, rerunnable prompt→output units.

## License

Released under the MIT License. See [LICENSE](LICENSE).
