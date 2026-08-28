# pomptty

A minimal, Chrome-flavored terminal emulator for Linux (macOS/Windows later),
built in Rust on [`egui`](https://github.com/emilk/egui) +
[`egui_term`](https://github.com/Harzu/egui_term) (which wraps
`alacritty_terminal` for VT parsing and the PTY).

## Status

Milestone 1 — **single-pane MVP**:

- one shell pane, GPU-rendered
- JSON config with live reload (theme, font size, shell, keybindings, window size)
- builtin themes (`solarized-dark` (default), `solarized-light`, `default-dark`, `default-light`), or an inline palette
- configurable app keybindings (font zoom, scrollback, clear, reload)
- mouse selection + copy/paste (`Ctrl+Shift+C` / `Ctrl+Shift+V`)

Planned next: tabs (M2), then persistent command history with `Ctrl+R` fuzzy
search backed by SQLite and OSC 133 shell integration (M3).

## Build & run

Needs a current Rust toolchain (1.92+). If you use `rustup`:

```sh
. "$HOME/.cargo/env"   # if ~/.cargo/bin isn't already on PATH
cargo run --release
```

## Configuration

On first run, `pomptty` writes a default config to:

```
~/.config/pomptty/config.json
```

Edit it while the app runs — changes are picked up automatically (or press
`Ctrl+Shift+R`). A malformed file is never overwritten; the previous config is
kept and an error is shown.

See [`config.example.json`](config.example.json) for every field.

| Field | Meaning |
|---|---|
| `font_family` | Path to a `.ttf`/`.otf` file, or `null` for the bundled monospace face. (Lookup by installed family name comes later.) |
| `font_size` | Terminal font size in points. |
| `scrollback_lines` | Reserved; wired to the backend in a later milestone. |
| `shell` | Shell to launch, or `null` for `$SHELL`. |
| `shell_args` | Extra arguments for the shell. |
| `theme` | An inline palette object, or a builtin name. |
| `keybindings` | Map of chord → action (see below). |
| `window` | Initial `width` / `height` in logical pixels. |

### Theme

The generated config writes the **full Solarized Dark palette inline** so every
color is right there to tweak:

```json
"theme": {
  "foreground": "#839496",
  "background": "#002b36",
  "cursor": "#93a1a1",
  "black": "#073642",
  "red": "#dc322f",
  "...": "..."
}
```

Editable keys: `foreground`, `background`, `cursor`, and the 16 ANSI colors
`black`…`white` / `bright_black`…`bright_white`. Any subset is fine — omitted
colors fall back to Solarized Dark.

To use a different builtin, replace the whole object with its name:
`"solarized-dark"`, `"solarized-light"`, `"default-dark"`, or `"default-light"`.
Deleting `config.json` regenerates the default (expanded Solarized Dark).

### Keybindings

Chords look like `ctrl+shift+t`, `ctrl+plus`, `shift+pageup`. Modifiers:
`ctrl`, `shift`, `alt`, `super`.

| Action | Default | Notes |
|---|---|---|
| `copy` / `paste` | `ctrl+shift+c` / `ctrl+shift+v` | handled by the terminal widget |
| `font-increase` | `ctrl+plus`, `ctrl+equals` | |
| `font-decrease` | `ctrl+minus` | |
| `font-reset` | `ctrl+0` | |
| `scroll-page-up` / `scroll-page-down` | `shift+pageup` / `shift+pagedown` | |
| `scroll-to-top` / `scroll-to-bottom` | `shift+home` / `shift+end` | |
| `clear` | `ctrl+shift+k` | sends `Ctrl+L` to the shell |
| `reload-config` | `ctrl+shift+r` | |
| `new-tab` / `close-tab` / `next-tab` / `prev-tab` | `ctrl+shift+t` / `w` / `pagedown` / `pageup` | reserved for M2 |
| `history-search` | `ctrl+r` | reserved for M3 (passed through to the shell for now) |
