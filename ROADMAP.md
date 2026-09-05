# pomptty roadmap

Feature tracker. `[x]` done · `[~]` partial · `[ ]` not started.

## Positioning

The "fast + GPU + tabs" lane is full (Alacritty, kitty, WezTerm, Ghostty, Rio).
pomptty's bet is elsewhere:

> **A local-first, open-source terminal that treats a shell session like a
> browser treats a page** — command *blocks* you can navigate, fold, copy and
> rerun; a Chrome-style omnibox; tabs and sessions that restore like browser
> windows — and a look that's genuinely beautiful out of the box.

Three pillars carry the identity: **M5 command blocks** (the flagship — Warp's
best idea, without the account / telemetry / AI / weight), **M6 browser-shaped
workflow** (the omnibox and everything around it), and **M4 stunning-by-default**
(the reason someone tries it at all). Everything else is table stakes or later.

Deliberate non-goals: no account, no telemetry, no AI-by-default (at most a
much-later opt-in, bring-your-own-key).

Both M3 and M5 depend on the shell-integration hook, so history is inert until
the user adds `eval "$(pomptty --print-integration <shell>)"` to their rc.

---

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
- [x] custom font from a file path or installed family name (`font_family`)
- [x] `TERM` / `COLORTERM` exported for the shell (`egui_term` doesn't)

## M2 — tabs ✅

- [x] `Vec<TerminalTab>` + active index + monotonic `TabId`
- [x] new / close / next / prev tab (keyboard + chrome)
- [x] `ctrl+1..9` jump to tab N (9 = last)
- [x] keybindings layer on defaults (new defaults appear automatically; `"disabled"` to suppress)
- [x] tab strip UI: pills, per-tab title (OSC 0/2), close button, middle-click close
- [x] `+` new-tab button, window title follows the active tab
- [x] close last tab → quit; tab whose shell exits is removed
- [x] "close tab" confirmation when a child process is still running
- [x] drag-to-reorder tabs (live shuffle: grabbed tab follows the cursor,
      neighbours slide, drop settles into the new slot)
- [x] new tab inherits the active tab's working directory
      (`egui_term` `working_directory` ← `/proc/<pid>/cwd`)

  _(Custom Chrome-style window decorations moved to M4 — they're a look, not a
  tabs feature.)_

## M3 — command history + `Ctrl+R` ✅

The escape-sequence route (OSC 133 / OSC 7) is a dead end — `vte` →
`alacritty_terminal` → `egui_term` surface neither. A **shell hook** records
history instead; cwd comes from `/proc/<shell_pid>/cwd`.

- [x] `pomptty --print-integration <bash|zsh|fish>` helper
- [x] shell hooks: one append-log line per command (command, cwd, exit, start,
      duration), inert unless `$POMPTTY` is set, `$?`-transparent
- [x] `CommandRecord` parse/format for the append-log line format
- [x] `LogStore`: read the `*.log` files, de-dup by command, rank by fuzzy score
      + recency/frequency (nucleo-matcher)
- [x] `ctrl+r` fuzzy-search overlay: ranked results, cwd + exit + "last run" shown
- [x] "this directory only" toggle
- [x] `Enter` inserts the command, `ctrl+Enter` runs it, `Esc` closes
- [x] `history` config section (`enabled`, `max_results`)
- [x] README "Shell integration" section

Later: SQLite + FTS5 store and `~/.bash_history` import (see _Later / polish_).

## M4 — stunning by default ★

The reason someone opens the screenshot and installs it.

**Slice 1 — chrome design pass + motion** (all in pomptty's egui layer):

- [x] `ui::style` design system: one palette-derived `Surfaces` shade set + the
      full `egui::Style` (widget states, selection, shadows, rounding, spacing,
      type scale) — replaces the old four-field `Visuals` tweak
- [x] refined tab strip: floating tabs, `surface`/accent treatment, faint
      dividers, raised-strip shadow; consumes `Surfaces`
- [x] cozy terminal padding (window edge → grid inset)
- [x] motion: tab-open spring, active-accent slide, toast slide+fade in/out,
      close-confirm modal fade-in; repaint only while animating
- [x] designed close-confirm modal (danger button, shadow, spacing)

- [x] **Slice 2 — frameless Chrome-style window** (opt-in, `window.decorations:
      "custom"`): tab strip is the title bar, min/max/close, strip drag region,
      resize edges, hairline border; the configured size is re-asserted on the
      first frame (some X11 WMs ignore it for an undecorated window). No client
      shadow / rounded corners on X11; Marco won't honour un-maximize on a CSD
      window (drag/resize still work).

**Slice 3 — glyph fallback + bell**:

- [x] Nerd / Powerline glyph fallback: an installed symbol font is auto-appended
      to the `Monospace` + `Proportional` fallback chains (via `fontdb`), so
      powerline prompts / devicons render instead of boxes. Zero binary weight.
- [x] terminal bell: brief accent flash + `RequestUserAttention` when unfocused

**Slice 4 — in-terminal polish** (needed forking `egui_term` — it rendered the
grid one `char` at a time with a single `FontId`, no cursor-style/hyperlink
exposure; forked into [`vendor/egui_term/`](vendor/egui_term)):

- [x] **per-cell bold / italic** (real weight faces, resolved per family via
      `fontdb`; falls back to the regular face rather than a synthetic
      embolden/oblique when no real one is installed)
- [x] smooth cursor (glide between cells) + block / beam / underline / blink /
      themed color from config — an app's own DECSCUSR style (`vim`'s
      insert-mode beam) still overrides the configured default
- [x] OSC 8 hyperlink hover affordance (plain hover, not `Ctrl`+hover); a bare
      typed URL still only underlines under `Ctrl`+hover
- [ ] **ligatures** — split out as its own future slice: real ligatures need
      OpenType GSUB shaping (`rustybuzz`), which egui/epaint's text system
      doesn't provide at all (cmap-only glyph lookup) — a full custom
      text-rendering subsystem, materially bigger than the rest of this slice

**Slice 5 — depth**: optional background blur / translucency; optional background
image with dimming + vignette; faint top-edge pane highlight.

## M5 — command blocks ★ (flagship)

Built on the M3 hook stream. Treat each prompt→command→output span as a unit.

- [ ] block model: fold the hook's start / exit / duration markers into ranges
      over the grid / scrollback
- [ ] per-block exit-status gutter (green / red / running) + hairline separators
- [ ] jump between prompts (`ctrl+↑` / `ctrl+↓`), "scroll to last prompt"
- [ ] select / copy / rerun a whole block; copy just its output
- [ ] fold long output to a summary line ("N lines hidden"), click to expand
- [ ] sticky header showing the currently-running command while its output scrolls
- [ ] share a block (plain text / styled) — local only, no upload

## M6 — browser-shaped workflow ★

- [x] **omnibox / command palette** (`ctrl+shift+p`): one input, four ranked
      sections — actions, tabs, history, recent directories — fuzzy-filtered
      together; `Enter` acts (run the action, switch tab, insert the command,
      `cd` there), `Ctrl+Enter` on a history row runs it. "Jump to a block" is
      out until M5 exists.
- [x] reopen closed tab (`ctrl+shift+t`: reopens the last closed tab in its old
      slot and directory, or opens a plain new tab when there's nothing to
      reopen), recently-closed list (right-click a tab → "Reopen closed")
- [x] tab search (`ctrl+shift+a`, fuzzy)
- [x] tab context menu: rename, duplicate, close others
- [x] tab colors: right-click a tab → Color → one of 8 fixed swatches (or
      None); survives duplicate, reopen-closed, and session restore
- [ ] tab groups (a named cluster of adjacent tabs that collapses/expands as
      one unit) — colors alone cover "tell tabs apart at a glance"; grouping
      is a bigger, separate feature
- [x] session restore: reopen tabs, directories, order, active tab and renames
      after quit / crash (`session.json`, periodic save + `on_exit`,
      `session.restore` config gate); scrollback replay not included
- [ ] profiles (per-profile shell / theme / cwd / font), profile picker in the omnibox
- [ ] settings UI (edit the config visually; still round-trips the JSON)

## M7 — splits & panes

- [ ] split a tab horizontally / vertically, focus navigation, resize
- [ ] per-pane cwd inheritance, "close pane" confirmation when busy
- [ ] broadcast input to all panes (toggle)

## M8 — platform & robustness

- [ ] macOS support (window decorations, `/proc` cwd alternative, `state_dir`),
      distributed via a Homebrew formula/cask
- [x] Windows support: builds and runs via `alacritty_terminal`'s existing
      ConPTY backend, defaults to `powershell.exe` when no shell is
      configured, ships as a plain `.exe` (no installer). `windows-latest` CI
      job guards against regressions. cwd/child-process detection and the
      `Ctrl+R` history hook remain Linux-only for now — see
      [Known issues](#known-issues).
- [ ] `.deb` package: `cargo-deb` metadata in `Cargo.toml` + a `.desktop` file +
      an icon (none exists yet) + a release CI job; no `-dev` packages needed
      at build time (windowing/GPU libs are `dlopen`'d at runtime)
- [ ] `.rpm` package: same shape via `cargo-generate-rpm`, mostly duplicate
      effort once the `.deb` metadata (desktop file, icon) exists
- [ ] non-fatal wgpu error handling + GPU / backend fallback (OOM panic on a 2 GB GPU)

## Later / polish

- [ ] session restore: optional scrollback replay (currently a fresh shell in
      the remembered directory, no output history)
- [ ] `scrollback_lines` actually wired to the backend
- [ ] scrollback search within a tab (browser-style find bar)
- [ ] history: SQLite + FTS5 store, compaction / pruning of the per-pid `*.log`
      files (drop-in behind the `HistoryStore` trait)
- [ ] history: import an existing `~/.bash_history` / `~/.zsh_history`
- [ ] configurable UI font / tab-bar density
- [ ] `TERM` value configurable; ship an `pomptty` terminfo entry
- [ ] sixel / kitty graphics protocol (inline images)
- [ ] AI: opt-in, off by default, bring-your-own-key, no data leaves the machine
      without an explicit action

## Known issues

- wgpu treats GPU errors as fatal; a texture-allocation failure panics the app
  (observed on an NVIDIA 940MX under VRAM pressure). Tracked in M8.
- On Windows: `shell_cwd`/`has_running_child` (`src/terminal/mod.rs`) are
  `/proc`-based and stay Linux-only stubs — no cheap Windows equivalent — so
  cwd-scoped history, new-tab-inherits-cwd, and the busy-tab close-warning
  silently no-op there instead of working. The `Ctrl+R` history overlay also
  has no data to show on Windows yet: the shell-integration hook only covers
  bash/zsh/fish, no PowerShell hook exists. Neither crashes anything — both
  are graceful degradations, tracked in M8.
