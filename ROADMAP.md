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
parity with mainstream terminals — the "don't lose me on day one" set;
**M10**, the heavier input/windowing features) or later.

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
- [x] **underline / strikethrough** — `\e[4m`, styled underlines (`\e[4:2m`
      double, `:3` undercurl, `:4` dotted, `:5` dashed), underline colour
      (`\e[58…m`) and `\e[9m` strikeout, rendered in the fork's `view.rs`
      (`underline_kind` + `push_text_decoration`; `alacritty_terminal`
      already parses and stores the flags, so the parser is untouched).
      neovim / helix LSP squiggles now render
- [x] **box-drawing / block / Powerline glyphs drawn by pomptty**, not the
      font (`vendor/egui_term/src/box_drawing.rs`): `U+2500..257F` lines
      (light + heavy, corners / tees / cross, partials), `U+2550..256C`
      double lines (junctions approximate, not per-char perfected),
      `U+2580..259F` block / shade / quadrant elements, `U+2571..2573`
      diagonals, `U+E0B0..E0B3` Powerline separators. Pixel-snapped filled
      rects → no sub-pixel gaps, exact scaling. `U+E0B4+` (rounded / slant
      powerline) and rounded corners `U+256D..2570` still fall through to
      the font
- [x] **"bold is bright"** — `bold_is_bright` config option maps a bold
      cell's normal palette colour (0–7) to its bright counterpart (8–15);
      `DIM` (`fg × 0.7`) and `DIM_BOLD` are excluded and still render dim

**Slice 5 — depth**:

- [x] **background image** — `window.background` `{ path, dim, vignette }`:
      the image is cover-fit behind the grid, blended toward the theme
      background by `dim`, with a soft edge `vignette` (four gradient
      strips). Painted into an opaque window, so it works on every backend
      (`load_image` / `paint_background` / `cover_uv` in `src/app.rs`; the
      grid skips its opaque sheet via `TerminalView::set_bg_opacity`)
- [x] **window translucency** — `window.opacity` (`0.05`–`1.0`). Active on
      Wayland / macOS / Windows; on X11 it's ignored with a warning
      (`config::window_translucency_supported`) because wgpu's Vulkan Xlib
      surface is opaque-only and would render the window black
- [x] **faint top-edge highlight** on the terminal pane — a 5%-alpha
      hairline so the pane reads as a raised sheet; always on
- backdrop **blur** is delegated to the compositor (KWin / picom
      `blur-background`, keyed on window app-id `pomptty`) — there is no
      portable winit/wgpu API for it, same as Alacritty

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

- [x] **omnibox / command palette** (`ctrl+shift+p`): one input, five ranked
      sections — actions, directory bookmarks, tabs, history, recent
      directories — fuzzy-filtered together; `Enter` acts (run the action,
      jump to the bookmark, switch tab, insert the command, `cd` there),
      `Ctrl+Enter` on a history row runs it. "Jump to a block" is out until
      M5 exists.
- [x] **directory bookmarks**: `bookmarks` config map (`name → path`, `~`
      expanded), each surfaced in the palette as `Jump to: <name>` →
      `cd`s the active tab; `Bookmarks: Create` in the palette saves the
      active tab's cwd back to `config.json` (`Config::Bookmarks`,
      `OmniItem::Bookmark`, `Action::BookmarkDir`)
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

- [~] macOS support: builds + tests on the `macos-latest` CI job (can't be
      cross-compiled from Linux — the SDK is proprietary). Process detection
      (cwd / busy-tab / superuser warning) now works via `ps` + `lsof`
      instead of `/proc` (`src/terminal/mod.rs`); `directories` already
      resolves `state_dir` to `~/Library/Application Support`; the frameless
      window builds unchanged. Still open: interactive tuning on real
      hardware, and a Homebrew formula/cask
- [x] Windows support: builds and runs via `alacritty_terminal`'s existing
      ConPTY backend, defaults to `powershell.exe` when no shell is
      configured, ships as a plain `.exe` (no installer). `windows-latest` CI
      job guards against regressions. cwd/child-process detection and the
      `Ctrl+R` history hook remain Linux/macOS-only for now — see
      [Known issues](#known-issues).
- [x] `.deb` package: `cargo-deb` metadata in `Cargo.toml`, a `.desktop`
      file, a man page and hicolor icons (16–256 + scalable) under
      `packaging/`, `make deb` for a local build, CI job builds + `lintian`s
      it and uploads it as a workflow artifact. Clean under `lintian` (bar
      the inherent `initial-upload-closes-no-bugs` info)
- [x] `.rpm` package: same `packaging/` assets via `cargo-generate-rpm`
      (pure Rust, no `rpmbuild` dependency), `make rpm` locally, same CI
      artifact treatment. Not yet content-verified as deeply as the `.deb`
      (no `rpm`/`rpm2cpio` available to inspect it here — `file` confirms a
      valid RPM, `cargo generate-rpm` built it from the same asset paths
      already verified via the `.deb`)
- [x] real GitHub Releases: `.github/workflows/release.yml` fires on a
      `vX.Y.Z` tag, builds the `.deb` / `.rpm` / Windows `.exe` / macOS
      tarball and publishes a Release with the `CHANGELOG.md` section as the
      notes. `CHANGELOG.md` (Keep a Changelog) + `RELEASING.md` document the
      version policy; a `verify` job rejects a tag that doesn't match
      `Cargo.toml` / the changelog
- [x] a proper man page — `packaging/pomptty.1` (hand-written roff),
      installed by both packages, clears the `no-manual-page` lint
- [x] an app icon — `packaging/pomptty.svg` redrawn (Solarized tile, active
      tab, `>` prompt + caret, readable at 16 px) and rasterized to the
      hicolor PNG sizes. Not a full brand system, but no longer a rough
      placeholder
- [x] GPU adapter choice: pomptty now requests a low-power adapter by default
      (`PowerPreference::LowPower` in `src/main.rs`, `WGPU_POWER_PREF` still
      overrides) and logs the chosen GPU at startup — this alone avoids the
      observed 2 GB-discrete-GPU OOM by not preferring that class of card.
- [x] security & safety: `SECURITY.md` (what pomptty does/doesn't do, files
      touched, how to verify, how to report), a `cargo deny` supply-chain job
      in CI (advisories/licenses/sources) with a committed `deny.toml`,
      `#![deny(unsafe_code)]` on pomptty's own crate with zero
      `#[allow(unsafe_code)]` exceptions (the spawned shell's `TERM` /
      `COLORTERM` / `POMPTTY*` env goes through `Command::env` via
      `BackendSettings::env`, not `std::env::set_var`), and a red terminal
      outline + tab dot while the shell (or `sudo -s` / `su` / a long
      `sudo …` under it) runs as `root` — `security.superuser_warning`
      config, Linux and macOS. Still open: Windows privilege detection,
      dangerous-command heuristics
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
      otherwise newlines are normalised to `\r`, matching Alacritty. A trailing
      newline is dropped in both modes, so a whole-line copy pastes at the
      prompt instead of pressing Return (a plain shell runs it directly; zsh's
      `bracketed-paste-magic` accepts it even inside the brackets).
      (`paste_payload` in the `egui_term` fork's `backend/mod.rs`.)
- [x] paste safety: a confirm dialog (line count + preview) before a multi-line
      `Ctrl+Shift+V` or middle-click paste reaches the shell — whether or not
      bracketed paste is on, since the buffered lines still run together on the
      next Return. A single line (after the trailing newline) passes straight
      through. `paste.confirm_multiline` config (default `true`)
- [x] **OSC 52 clipboard**: apps setting the system clipboard (tmux, neovim,
      `vim` `+clipboard`) — the main way to copy *out of* an SSH session.
      `\e]52;c;…` → `CLIPBOARD`, `\e]52;p/s;…` → the X11 primary selection
      (`PtyEvent::ClipboardStore` routed by `ClipboardType` in `src/app.rs`).
      The *read* direction (`\e]52;c;?`) is a config opt-in,
      `clipboard.osc52_read` (default off, matching Alacritty), answered via
      `arboard` + `BackendCommand::Report`
- [x] **OSC 4 / 10 / 11 / 12 / 104 / 110-112 dynamic colors**: apps *setting*
      the palette / fg / bg / cursor at runtime (neovim colorschemes,
      `dircolors`) — `RenderableContent::colors` + `resolve_color` in the
      fork; resets fall back to the configured theme. The *query* form
      (`\e]11;?` → `PtyEvent::ColorRequest`) is answered from the runtime
      override or, failing that, the theme (`egui_term::theme_rgb` →
      `BackendCommand::Report`)
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
- [x] **open scrollback in an editor** — `open-scrollback` action /
      "Terminal: Open Scrollback in Editor" in the palette dumps the tab's
      grid + scrollback to a temp file and hands it to the system's default
      handler (`egui_term::TerminalBackend::scrollback_text` + the `open`
      crate). A GUI app can't host a TUI pager, so this is the pragmatic
      version of kitty's `edit-in-*`; "just the last command's output" waits
      on the M5 block model
- [x] **bind a key to raw bytes / an escape sequence** — the `key_sends`
      config map (chord → string, with `\e` / `\xNN` / `\u{NNNN}` / `\n`
      etc.), checked in `handle_bindings` after the app actions
- [x] **desktop notification when a long command finishes** while pomptty is
      unfocused, minimised, or on another tab. A background thread watches the
      shell-hook logs (independent of the UI loop, so it fires while
      minimised); `notifications.long_command_secs` config, default 300
      (`0` off). Needs the shell-integration hook.
- [x] **IME / dead-key / compose input** — the fork handles
      `egui::Event::Ime(Commit)` (→ write the committed string to the PTY),
      so the compose key and X11 dead keys reach the shell. No inline
      preedit rendering yet (CJK candidate text commits on selection).

## M10 — advanced input & windowing

Promoted out of M9: each is a feature in its own right, not a quick
table-stakes fix.

- [ ] **keyboard scrollback / copy-mode**: scroll, select, and search the
      scrollback with the keyboard (vi-style motions), no mouse — kitty /
      WezTerm / tmux all have this
- [ ] **kitty keyboard protocol / `CSI u`** (`\e[?…u`): disambiguate
      `Ctrl+I`/`Tab`, `Ctrl+[`/`Esc`, expose key-release and more modifiers —
      neovim, helix, and tmux all want it. `alacritty_terminal` tracks the
      mode flags; the frontend has to *encode* key events to match (≈
      alacritty's `input/keyboard.rs`)
- [ ] **multiple OS windows**, not just tabs — drag a tab out,
      `ctrl+shift+n`; needs egui multi-viewport and moving a PTY-backed tab
      between windows

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
- [ ] startup latency: the system-font scan (~100 ms) is now deferred to a
      background thread; the remaining ~300 ms to first paint is wgpu
      device/surface creation (~210 ms) + egui's first font-atlas / pipeline
      build (~85 ms), both largely graphics-stack costs

## Known issues

- wgpu treats a mid-run GPU error as fatal; a buffer/texture-allocation
  failure panics the app (the panic is inside `egui-wgpu`, no config hook).
  Mitigated by defaulting to a low-power adapter (see M8), so the
  VRAM-pressure trigger is much less likely, but a real fix needs an
  `egui-wgpu` fork. Tracked in M8.
- On Windows: `shell_cwd`/`has_running_child` (`src/terminal/mod.rs`) have no
  cheap equivalent and stay stubs — so cwd-scoped history,
  new-tab-inherits-cwd, and the busy-tab close-warning silently no-op there
  instead of working. The `Ctrl+R` history overlay also has no data on
  Windows: the shell-integration hook only covers bash/zsh/fish, no
  PowerShell hook exists. Neither crashes anything — graceful degradations,
  tracked in M8. macOS uses `ps`/`lsof` for these and works, but has not been
  exercised on real hardware yet.
- The `.deb`/`.rpm` `maintainer`/`copyright` metadata is the repo owner's
  info baked in at packaging time.
