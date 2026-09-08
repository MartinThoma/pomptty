# Changelog

All notable changes to pomptty are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). See
[RELEASING.md](RELEASING.md) for the release process.

## [Unreleased]

### Added

- Directory bookmarks: a `bookmarks` config map (`name → path`). Each entry
  shows in the command palette as `Jump to: <name>` and `cd`s the active tab
  there; a leading `~` in the path is expanded to the home directory. The
  palette's `Bookmarks: Create` saves the active tab's current directory
  (prompting for a name) straight into `config.json`.

### Changed

- pomptty's own crate is now 100% safe Rust — the last `unsafe` blocks (the
  startup `std::env::set_var` calls) are gone. The spawned shell's `TERM` /
  `COLORTERM` / `POMPTTY` / `POMPTTY_HISTORY_DIR` are passed to the child
  process via `Command::env`, not set on pomptty's process. `#![deny(unsafe_code)]`
  now has zero `#[allow]` exceptions.

## [0.1.1] - 2026-09-07

### Added

- Command palette: "Help: About pomptty" — a dialog with the version,
  repository and license, and a "Copy version" button. Also bindable as
  `"about"`.

### Fixed

- Paste no longer runs on its own. A trailing newline is stripped from every
  paste, so a whole-line copy (`git status\n`) lands at the prompt instead of
  executing — including inside bracketed paste, where zsh's
  `bracketed-paste-magic` would otherwise accept it. The multi-line confirm
  dialog now also appears for bracketed-paste shells, since the buffered lines
  still run together on the next Return.

## [0.1.0]

First tagged release.

### Terminal

- GPU-rendered VT emulation via `alacritty_terminal`, on `egui` / `wgpu`.
- Tabs: new / close / reorder (drag), `Ctrl+1..9`, next/prev, rename,
  duplicate, close-others, per-tab colors, middle-click close, "reopen
  closed tab".
- Command history: `Ctrl+R` fuzzy overlay fed by a shell-integration hook
  (`pomptty --print-integration bash|zsh|fish`), scoped to "this directory".
- Command palette (`Ctrl+Shift+P`) over actions, tabs, history and recent
  directories.
- Session restore: tabs, directories, order, renames.
- Selection & clipboard: mouse select + copy/paste, word/line select,
  `Ctrl+Alt`+drag block selection, X11 primary selection (select-to-copy,
  middle-click paste), bracketed paste with a multi-line-paste confirm,
  OSC 52 copy (and opt-in read).
- OSC 4/10/11/12 dynamic colors (set and query), OSC 8 hyperlinks.
- `key_sends` config for binding a chord to raw bytes / an escape sequence.
- "Open scrollback in editor" action.
- Long-running-command desktop notification (configurable threshold).
- IME / dead-key / compose input.

### Appearance

- Palette-derived design system; frameless "custom" window decorations
  (opt-in) with pomptty's own tab strip as the title bar.
- Nerd/Powerline glyph fallback; per-cell bold/italic real faces; smooth,
  shaped, themed cursor.
- Underline (single / double / undercurl / dotted / dashed) with SGR 58
  underline color, and strikethrough.
- pomptty-drawn box-drawing / block / shade / Powerline glyphs (crisp at any
  size).
- `bold_is_bright` option.
- Optional background image (dim + vignette) and, on Wayland / macOS /
  Windows, window translucency; faint top-edge pane highlight.

### Platform & packaging

- Linux and Windows (ConPTY) supported; macOS builds and has `ps`/`lsof`
  process detection (cwd, busy-tab, superuser warning).
- `.deb` and `.rpm` via `cargo-deb` / `cargo-generate-rpm`; `make windows`
  cross-compiles the `.exe`; a man page and hicolor icons ship in the
  packages.
- Low-power GPU adapter by default.

### Security

- `SECURITY.md`, a `cargo deny` supply-chain CI job, `#![deny(unsafe_code)]`
  on pomptty's own crate, and a red superuser warning border.

[Unreleased]: https://github.com/MartinThoma/pomptty/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/MartinThoma/pomptty/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/MartinThoma/pomptty/releases/tag/v0.1.0
