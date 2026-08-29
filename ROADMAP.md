# pomptty roadmap

Feature tracker. `[x]` done · `[~]` partial · `[ ]` not started.

## M1 — single-pane MVP ✅

- [x] eframe/egui + wgpu app window
- [x] one PTY-backed terminal pane (`egui_term` / `alacritty_terminal`)
- [x] JSON config at `~/.config/pomptty/config.json`, created with defaults on first run
- [x] tolerant parsing (partial config, unknown fields); malformed file never overwritten
- [x] live config reload (file watcher + `ctrl+shift+r`)
- [x] themes: builtins (`solarized-dark` default, `solarized-light`, `default-dark`, `default-light`) or inline palette
- [x] default config writes the full palette inline so colors are easy to edit
- [x] egui chrome tinted to match terminal background
- [x] configurable app keybindings (chord string → action)
- [x] font zoom (`ctrl +/-/0`), scrollback keys, clear
- [x] mouse selection + copy/paste (via `egui_term`)
- [x] custom font from a file path (`font_family`)

## M2 — tabs 🚧

- [x] `Vec<TerminalTab>` + active index + monotonic `TabId`
- [x] new / close / next / prev tab (keyboard + chrome)
- [x] `ctrl+1..9` jump to tab N (9 = last)
- [x] keybindings layer on defaults (new defaults appear automatically; `"disabled"` to suppress)
- [x] tab strip UI: pills, per-tab title (OSC 0/2), close button, middle-click close
- [x] `+` new-tab button
- [x] window title follows the active tab
- [x] close last tab → quit; tab whose shell exits is removed
- [ ] drag-to-reorder tabs
- [ ] custom window decorations (frameless + integrated title/tab bar, Chrome-style;
      window drag region, min/max/close buttons, resize borders)
- [ ] new tab inherits the active tab's working directory (needs OSC 7 — see M3)
- [x] "close tab" confirmation when a child process is still running

## M3 — command history + `Ctrl+R`

- [ ] shell integration snippets (`bash`/`zsh`/`fish`) emitting OSC 133 A/B/C/D + OSC 7
- [ ] `pomptty --print-integration <shell>` helper
- [ ] parse OSC 133/OSC 7 from the terminal event stream → `CommandRecord`
- [ ] SQLite store at `~/.local/state/pomptty/history.db` (`commands` table, optional FTS5)
- [ ] `record()` on command completion (command, cwd, exit code, duration, session)
- [ ] `ctrl+r` fuzzy-search overlay (nucleo): ranked results, cwd + exit shown
- [ ] "this directory only" filter toggle
- [ ] `Enter` inserts the command, `ctrl+Enter` runs it, `Esc` closes
- [ ] de-dup / frequency weighting

## Later / polish

- [ ] system font lookup by family name (fontdb) instead of file path only
- [ ] `scrollback_lines` actually wired to the backend
- [ ] cursor color / style (block/bar/underline, blink) from config
- [ ] split panes
- [ ] profiles (per-profile shell/theme/cwd)
- [ ] tab context menu (rename, duplicate, close others)
- [ ] scrollback search within a tab (browser-style find)
- [ ] configurable UI font / tab-bar density
- [ ] non-fatal wgpu error handling + GPU/backend fallback (seen: `Out of Memory` panic on a 2 GB GPU)
- [ ] macOS / Windows support
- [ ] settings UI (edit config without the JSON file)
- [ ] session restore (reopen tabs / cwd)
- [ ] bell / urgency handling
- [ ] hyperlink (OSC 8) support

## Known issues

- wgpu treats GPU errors as fatal; a texture-allocation failure panics the app
  (observed on an NVIDIA 940MX under VRAM pressure).
- `font_family` only accepts a path to a font file, not an installed family name.
