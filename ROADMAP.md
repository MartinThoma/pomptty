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
(the reason someone tries it at all). Everything else is table stakes (**M9**,
parity with mainstream terminals — the "don't lose me on day one" set) or
later.

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
- [ ] **underline / strikethrough** — the fork renders *no* underline today
      (only hyperlink hover). Add `\e[4m`, plus styled underlines
      (`\e[4:2m` double, `:3` undercurl, `:4` dotted, `:5` dashed) and
      underline color (`\e[58…m`) — neovim/helix LSP squiggles depend on
      this — and `\e[9m` strikethrough
- [ ] **box-drawing / block / Powerline glyphs drawn by pomptty**, not the
      font — pixel-perfect `─│┌┘`, shade blocks, and powerline separators at
      any size/font, the way kitty/WezTerm/Alacritty/Ghostty do. Fits
      "stunning by default"
- [ ] **"bold is bright"** option (map bold text onto the bright palette, as
      many terminals do); confirm `DIM` / `DIM_BOLD` render right

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
      distributed via a Homebrew formula/cask. `macos-latest` CI job now
      builds + tests on real Apple hardware (can't be cross-compiled from
      Linux — the SDK is proprietary, unlike `mingw-w64` for Windows), but
      nothing macOS-specific has been tuned or run interactively yet
- [x] Windows support: builds and runs via `alacritty_terminal`'s existing
      ConPTY backend, defaults to `powershell.exe` when no shell is
      configured, ships as a plain `.exe` (no installer). `windows-latest` CI
      job guards against regressions. cwd/child-process detection and the
      `Ctrl+R` history hook remain Linux-only for now — see
      [Known issues](#known-issues).
- [x] `.deb` package: `cargo-deb` metadata in `Cargo.toml`, a `.desktop` file
      + placeholder icon under `packaging/` ([Known issues](#known-issues)),
      `make deb` for a local build, CI job uploads it as a workflow artifact
      on every push/PR/manual run. Lints clean under `lintian` (bar two minor
      style warnings — synopsis starts with "A", no man page yet)
- [x] `.rpm` package: same `packaging/` assets via `cargo-generate-rpm`
      (pure Rust, no `rpmbuild` dependency), `make rpm` locally, same CI
      artifact treatment. Not yet content-verified as deeply as the `.deb`
      (no `rpm`/`rpm2cpio` available to inspect it here — `file` confirms a
      valid RPM, `cargo generate-rpm` built it from the same asset paths
      already verified via the `.deb`)
- [ ] real GitHub Releases with `.deb`/`.rpm`/`.exe` attached on a tag push —
      needs a version-tagging/changelog policy this repo doesn't have yet;
      `workflow_dispatch` artifacts cover "get a build right now" meanwhile
- [ ] a proper man page (`no-manual-page`, flagged by `lintian`)
- [ ] a considered app icon/logo — `packaging/pomptty.svg` is a functional
      placeholder (Solarized-Dark terminal-window glyph), not real branding
- [x] GPU adapter choice: pomptty now requests a low-power adapter by default
      (`PowerPreference::LowPower` in `src/main.rs`, `WGPU_POWER_PREF` still
      overrides) and logs the chosen GPU at startup — this alone avoids the
      observed 2 GB-discrete-GPU OOM by not preferring that class of card.
- [ ] non-fatal wgpu error handling: making an actual mid-run wgpu error (the
      hard `panic!()` in `egui-wgpu`'s renderer when a buffer allocation
      fails) recoverable instead of crashing — needs an `egui-wgpu` fork,
      deliberately deferred.

## M9 — parity with mainstream terminals

The "don't lose me on day one" set — table stakes that Alacritty / kitty /
WezTerm / iTerm2 / Windows Terminal users reach for and would currently miss.
None of these are the reason to pick pomptty; all of them are reasons someone
bounces off it.

- [x] **bracketed paste**: paste is wrapped in `\e[200~…\e[201~` when the app
      set `\e[?2004h` (embedded `ESC`/`ST` stripped so it can't break out);
      otherwise newlines are normalised to `\r`, matching Alacritty. Multi-line
      paste no longer auto-runs in a shell. (`paste_payload` in the
      `egui_term` fork's `view.rs`.)
- [x] paste safety: a confirm dialog (line count + preview) before pasting
      newline-containing text into a shell that hasn't turned on bracketed
      paste — where each line runs on arrival. Covers `Ctrl+Shift+V` and
      middle-click; single-line pastes and bracketed-paste apps pass straight
      through. `paste.confirm_multiline` config (default `true`)
- [~] **OSC 52 clipboard**: apps setting the system clipboard (tmux, neovim,
      `vim` `+clipboard`) now works — the main way to copy *out of* an SSH
      session (`PtyEvent::ClipboardStore` → `ctx.copy_text` in `src/app.rs`;
      `alacritty_terminal`'s default `Osc52::OnlyCopy` already gates it to the
      copy direction). Still open: routing `p`/`s` requests to the X11 primary
      selection (needs the primary-selection work below), and a config opt-in
      for the *read* direction
- [~] **OSC 4 / 10 / 11 / 12 / 104 / 110-112 dynamic colors**: apps *setting*
      the palette / fg / bg / cursor at runtime now works (neovim
      colorschemes, `dircolors`) — `RenderableContent::colors` +
      `resolve_color` in the `egui_term` fork; resets fall back to the
      configured theme. Still open: the *query* form (`OSC 10;?` →
      `Event::ColorRequest`), which needs the theme's defaults reachable from
      the backend thread to answer for un-overridden slots
- [x] **primary selection** (X11/Wayland): the mouse selection is mirrored
      onto `PRIMARY`, middle-click pastes it (via bracketed paste, so
      multi-line is safe). `arboard` directly, since egui only exposes
      `CLIPBOARD`; `src/primary_selection.rs`, a no-op stub off Linux.
      Middle-click still isn't *forwarded* to apps in mouse mode — separate
      gap
- [x] **rectangular / block selection**: `Ctrl`+`Alt`+drag (plain `Alt`+drag
      is the WM's move-window gesture on most X11 setups, e.g. Marco). Also
      fixed a pre-existing fork bug where *any* multi-row copy (block, plain,
      `Ctrl+Shift+C`) was mashed onto one line — `selectable_content()` now
      uses `alacritty_terminal`'s own `selection_to_string()`
- [ ] **keyboard scrollback / copy-mode**: scroll, select, and search the
      scrollback with the keyboard (vi-style motions), no mouse — kitty /
      WezTerm / tmux all have this
- [ ] **open scrollback (or the last command's output) in `$EDITOR` / `$PAGER`**
      — kitty's `edit-in-*`, one of its most-loved features; cheap once the
      M5 block model exists
- [ ] **kitty keyboard protocol / `CSI u`** (`\e[?…u`): disambiguate
      `Ctrl+I`/`Tab`, `Ctrl+[`/`Esc`, expose key-release and more modifiers —
      neovim, helix, and tmux all want it
- [ ] **bind a key to raw bytes / an escape sequence** — keybindings today
      only map to named app actions, not "send `\e[1;5D`" or arbitrary input
- [ ] **desktop notification when a long command finishes** in an unfocused
      tab/window (the shell hook already knows command + duration; distinct
      from the bell)
- [ ] **multiple OS windows**, not just tabs — drag a tab out, `ctrl+shift+n`
- [ ] **IME / dead-key / compose input** — matters for non-US layouts
      (umlauts, accents); verify egui's IME path works through the grid

## Later / polish

- [ ] session restore: optional scrollback replay (currently a fresh shell in
      the remembered directory, no output history)
- [ ] `scrollback_lines` actually wired to the backend
- [ ] scrollback search within a tab (browser-style find bar)
- [ ] history: SQLite + FTS5 store, compaction / pruning of the per-pid `*.log`
      files (drop-in behind the `HistoryStore` trait)
- [ ] history: import an existing `~/.bash_history` / `~/.zsh_history`
- [ ] configurable UI font / tab-bar density
- [ ] adjustable cell metrics: line height, letter spacing, window padding
- [ ] ordered font-fallback *list* in config (not just the one auto Nerd Font)
- [ ] follow the system light/dark preference; switch theme at runtime
- [ ] import iTerm2 `.itermcolors` / base16 / Alacritty themes; a theme gallery
- [ ] Unicode width / wide-char / emoji / grapheme-cluster correctness pass
- [ ] trim trailing whitespace on copy; "copy as styled" (HTML/RTF)
- [ ] `TERM` value configurable; ship an `pomptty` terminfo entry
- [ ] sixel / kitty graphics protocol (inline images)
- [ ] AI: opt-in, off by default, bring-your-own-key, no data leaves the machine
      without an explicit action

## Known issues

- wgpu treats a mid-run GPU error as fatal; a buffer/texture-allocation
  failure panics the app (the panic is inside `egui-wgpu`, no config hook).
  Mitigated by defaulting to a low-power adapter (see M8), so the
  VRAM-pressure trigger is much less likely, but a real fix needs an
  `egui-wgpu` fork. Tracked in M8.
- On Windows: `shell_cwd`/`has_running_child` (`src/terminal/mod.rs`) are
  `/proc`-based and stay Linux-only stubs — no cheap Windows equivalent — so
  cwd-scoped history, new-tab-inherits-cwd, and the busy-tab close-warning
  silently no-op there instead of working. The `Ctrl+R` history overlay also
  has no data to show on Windows yet: the shell-integration hook only covers
  bash/zsh/fish, no PowerShell hook exists. Neither crashes anything — both
  are graceful degradations, tracked in M8.
- The `.deb`/`.rpm` packages (`packaging/`) use a hand-drawn placeholder
  icon and have no man page yet; the `.deb` maintainer/copyright metadata is
  the repo owner's info baked in at packaging time. Tracked in M8.
