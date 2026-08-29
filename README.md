# pomptty

A minimal, Chrome-flavored terminal emulator for Linux (macOS/Windows later),
built in Rust on [`egui`](https://github.com/emilk/egui) +
[`egui_term`](https://github.com/Harzu/egui_term) (which wraps
`alacritty_terminal` for VT parsing and the PTY).

## Status

- GPU-rendered shell panes with **tabs** (keyboard + mouse; window title follows
  the active tab)
- JSON config with live reload (theme, font size, shell, keybindings, window size)
- builtin themes (`solarized-dark` (default), `solarized-light`, `default-dark`,
  `default-light`), or an inline palette
- configurable app keybindings (tabs, font zoom, scrollback, clear, reload)
- mouse selection + copy/paste (`Ctrl+Shift+C` / `Ctrl+Shift+V`)

See [ROADMAP.md](ROADMAP.md) for what's done and what's next (up next: persistent
command history with `Ctrl+R` fuzzy search, M3).

## Build & run

Needs a current Rust toolchain (1.92+). If you use `rustup`:

```sh
. "$HOME/.cargo/env"   # if ~/.cargo/bin isn't already on PATH
cargo run --release
```

Or use the Makefile (it puts `~/.cargo/bin` on `PATH` for you):

```sh
make run       # debug build + run
make build     # debug build
make release   # optimized build
make test      # test suite
make lint      # rustfmt --check + clippy
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

`"keybindings"` maps a chord string to an action name. Chords look like
`ctrl+shift+t`, `ctrl+plus`, `shift+pageup`; modifiers are `ctrl`, `shift`,
`alt`, `super`.

Your `keybindings` block is **layered on the built-in defaults** — an entry
overrides or adds a chord, and new default shortcuts in a future version appear
automatically. To turn a default off, bind it to `"disabled"`
(e.g. `"ctrl+r": "disabled"`).

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
| `new-tab` | `ctrl+shift+t` | |
| `close-tab` | `ctrl+shift+w` | closing the last tab quits |
| `next-tab` / `prev-tab` | `ctrl+tab` / `ctrl+shift+tab` (also `ctrl+shift+pagedown` / `pageup`) | wraps around |
| `goto-tab-1` … `goto-tab-9` | `ctrl+1` … `ctrl+9` | jump to that tab; `goto-tab-9` = last tab |
| `history-search` | `ctrl+r` | reserved for M3 (passed through to the shell for now) |
| `disabled` | — | suppresses a default binding |

Tabs can also be managed with the mouse: click a tab to focus it, click `✕` or
middle-click to close, `+` to open a new one.

Closing a tab that still has a process running in it (an editor, `ssh`, a build)
pops up a confirmation first. If that process finishes while the prompt is open,
the tab closes as originally asked.
