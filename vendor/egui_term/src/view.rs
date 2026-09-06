use std::time::Duration;

use alacritty_terminal::index::Point as TerminalGridPoint;
use alacritty_terminal::term::cell;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use egui::epaint::RectShape;
use egui::Color32;
use egui::Modifiers;
use egui::MouseWheelUnit;
use egui::Shape;
use egui::Widget;
use egui::{Align2, Painter, Pos2, Rect, Response, Stroke, StrokeKind, Vec2};
use egui::{CornerRadius, Key};
use egui::{FontId, Id, PointerButton};

use crate::backend::BackendCommand;
use crate::backend::TerminalBackend;
use crate::backend::{LinkAction, MouseButton, SelectionType};
use crate::bindings::Binding;
use crate::bindings::{BindingAction, BindingsLayout, InputKind};
use crate::box_drawing;
use crate::font::TerminalFont;
use crate::theme::TerminalTheme;
use crate::types::Size;

const EGUI_TERM_WIDGET_ID_PREFIX: &str = "egui_term::instance::";

#[derive(Debug, Clone)]
enum InputAction {
    BackendCall(BackendCommand),
    WriteToClipboard(String),
    Ignore,
}

#[derive(Clone, Default)]
pub struct TerminalViewState {
    is_dragged: bool,
    scroll_pixels: f32,
    current_mouse_position_on_grid: TerminalGridPoint,
}

pub struct TerminalView<'a> {
    widget_id: Id,
    has_focus: bool,
    size: Vec2,
    backend: &'a mut TerminalBackend,
    font: TerminalFont,
    theme: TerminalTheme,
    bindings_layout: BindingsLayout,
    /// Draw bold text with the bright palette entry (SGR 1 → colours 8–15).
    bold_is_bright: bool,
    /// Opacity of the terminal's *default* background. `< 1.0` means the
    /// caller paints a translucent sheet / background image behind the grid,
    /// so the opaque full-rect fill here is skipped. Colored cell backgrounds
    /// stay opaque regardless.
    bg_opacity: f32,
}

impl Widget for TerminalView<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (layout, painter) = ui.allocate_painter(self.size, egui::Sense::click());

        let widget_id = self.widget_id;
        let mut state = ui.memory(|m| {
            m.data
                .get_temp::<TerminalViewState>(widget_id)
                .unwrap_or_default()
        });

        self.focus(&layout)
            .resize(&layout)
            .process_input(&layout, &mut state)
            .show(&mut state, &layout, &painter);

        ui.memory_mut(|m| m.data.insert_temp(widget_id, state));
        layout
    }
}

impl<'a> TerminalView<'a> {
    pub fn new(ui: &mut egui::Ui, backend: &'a mut TerminalBackend) -> Self {
        let widget_id =
            ui.make_persistent_id(format!("{}{}", EGUI_TERM_WIDGET_ID_PREFIX, backend.id()));

        Self {
            widget_id,
            has_focus: false,
            size: ui.available_size(),
            backend,
            font: TerminalFont::default(),
            theme: TerminalTheme::default(),
            bindings_layout: BindingsLayout::new(),
            bold_is_bright: false,
            bg_opacity: 1.0,
        }
    }

    #[inline]
    pub fn set_theme(mut self, theme: TerminalTheme) -> Self {
        self.theme = theme;
        self
    }

    #[inline]
    pub fn set_font(mut self, font: TerminalFont) -> Self {
        self.font = font;
        self
    }

    #[inline]
    pub fn set_focus(mut self, has_focus: bool) -> Self {
        self.has_focus = has_focus;
        self
    }

    #[inline]
    pub fn set_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    #[inline]
    pub fn set_bold_is_bright(mut self, yes: bool) -> Self {
        self.bold_is_bright = yes;
        self
    }

    #[inline]
    pub fn set_bg_opacity(mut self, opacity: f32) -> Self {
        self.bg_opacity = opacity;
        self
    }

    #[inline]
    pub fn add_bindings(mut self, bindings: Vec<(Binding<InputKind>, BindingAction)>) -> Self {
        self.bindings_layout.add_bindings(bindings);
        self
    }

    fn focus(self, layout: &Response) -> Self {
        if self.has_focus {
            layout.request_focus();
        } else {
            layout.surrender_focus();
        }

        self
    }

    fn resize(self, layout: &Response) -> Self {
        self.backend.process_command(BackendCommand::Resize(
            Size::from(layout.rect.size()),
            self.font.font_measure(&layout.ctx),
        ));

        self
    }

    fn process_input(self, layout: &Response, state: &mut TerminalViewState) -> Self {
        // Keyboard input only needs the grid to hold focus. Mouse events
        // additionally need the pointer over the grid (or a drag in progress),
        // so a click on the tab strip doesn't start a selection.
        if !layout.has_focus() {
            state.is_dragged = false;
            return self;
        }
        let pointer_ok = layout.contains_pointer() || state.is_dragged;

        let modifiers = layout.ctx.input(|i| i.modifiers);
        let events = layout.ctx.input(|i| i.events.clone());
        for event in events {
            let mut input_actions = vec![];

            match event {
                egui::Event::Text(_)
                | egui::Event::Key { .. }
                | egui::Event::Copy
                | egui::Event::Paste(_) => input_actions.push(process_keyboard_event(
                    event,
                    self.backend,
                    &self.bindings_layout,
                    modifiers,
                )),
                // Composed input (dead keys, compose key, CJK IME): the
                // platform hands us the finished string on commit.
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => input_actions.push(
                    InputAction::BackendCall(BackendCommand::Write(text.into_bytes())),
                ),
                egui::Event::MouseWheel { unit, delta, .. } if pointer_ok => input_actions.push(
                    process_mouse_wheel(state, self.font.font_type().size, unit, delta),
                ),
                egui::Event::PointerButton {
                    button,
                    pressed,
                    modifiers,
                    pos,
                    ..
                } if pointer_ok => input_actions.push(process_button_click(
                    state,
                    layout,
                    self.backend,
                    &self.bindings_layout,
                    button,
                    pos,
                    &modifiers,
                    pressed,
                )),
                egui::Event::PointerMoved(pos) if pointer_ok => {
                    input_actions = process_mouse_move(state, layout, self.backend, pos, &modifiers)
                }
                _ => {}
            };

            for action in input_actions {
                match action {
                    InputAction::BackendCall(cmd) => {
                        self.backend.process_command(cmd);
                    }
                    InputAction::WriteToClipboard(data) => {
                        layout.ctx.copy_text(data);
                    }
                    InputAction::Ignore => {}
                }
            }
        }

        self
    }

    fn show(self, state: &mut TerminalViewState, layout: &Response, painter: &Painter) {
        let content = self.backend.sync();
        let layout_min = layout.rect.min;
        let layout_max = layout.rect.max;
        let cell_height = content.terminal_size.cell_height as f32;
        let cell_width = content.terminal_size.cell_width as f32;
        let colors = &content.colors;
        let global_bg = resolve_color(&self.theme, colors, Color::Named(NamedColor::Background));

        // The opaque background sheet is skipped when the caller is drawing a
        // translucent window / background image behind the grid.
        let mut shapes = Vec::new();
        if self.bg_opacity >= 1.0 {
            shapes.push(Shape::Rect(RectShape::filled(
                Rect::from_min_max(layout_min, layout_max),
                CornerRadius::ZERO,
                global_bg,
            )));
        }

        // A real OSC 8 hyperlink under the pointer, if any, found once so the
        // per-cell loop below can just compare hyperlink ids. This is plain
        // hover — no modifier needed — unlike the regex-guessed bare-URL
        // underline (`hovered_hyperlink`) below, which still needs Ctrl+hover.
        let hovered_link_id = content
            .grid
            .display_iter()
            .find(|indexed| indexed.point == state.current_mouse_position_on_grid)
            .and_then(|indexed| indexed.cell.hyperlink())
            .map(|link| link.id().to_owned());

        // Captured at the cursor's cell during the loop, drawn after it so the
        // cursor (which may be gliding between cells) is always on top.
        let mut cursor_target: Option<(Pos2, f32)> = None;
        let mut cursor_glyph: Option<(char, FontId, Pos2)> = None;

        for indexed in content.grid.display_iter() {
            let flags = indexed.cell.flags;
            let is_wide_char_spacer = flags.contains(cell::Flags::WIDE_CHAR_SPACER);
            if is_wide_char_spacer {
                continue;
            }

            let is_app_cursor_mode = content.terminal_mode.contains(TermMode::APP_CURSOR);
            let is_wide_char = flags.contains(cell::Flags::WIDE_CHAR);
            let is_bold = flags.intersects(cell::Flags::BOLD | cell::Flags::DIM_BOLD);
            let is_italic = flags.contains(cell::Flags::ITALIC);
            let is_inverse = flags.contains(cell::Flags::INVERSE);
            let is_dim = flags.intersects(cell::Flags::DIM | cell::Flags::DIM_BOLD);
            let is_selected = content
                .selectable_range
                .is_some_and(|r| r.contains(indexed.point));
            let is_regex_hovered_hyperlink = content.hovered_hyperlink.as_ref().is_some_and(|r| {
                r.contains(&indexed.point) && r.contains(&state.current_mouse_position_on_grid)
            });
            let is_osc8_hovered_hyperlink = hovered_link_id.as_ref().is_some_and(|id| {
                indexed
                    .cell
                    .hyperlink()
                    .as_ref()
                    .is_some_and(|link| link.id() == id)
            });
            let is_hovered_hyperling = is_regex_hovered_hyperlink || is_osc8_hovered_hyperlink;

            let x = layout_min.x + (cell_width * indexed.point.column.0 as f32);
            let line_num = indexed.point.line.0 + content.grid.display_offset() as i32;
            let y = layout_min.y + (cell_height * line_num as f32);

            let fg_spec = if self.bold_is_bright && is_bold && !is_dim {
                brighten(indexed.fg)
            } else {
                indexed.fg
            };
            let mut fg = resolve_color(&self.theme, colors, fg_spec);
            let mut bg = resolve_color(&self.theme, colors, indexed.bg);
            let cell_width = if is_wide_char {
                cell_width * 2.0
            } else {
                cell_width
            };

            if is_dim {
                fg = fg.linear_multiply(0.7);
            }

            if is_inverse || is_selected {
                std::mem::swap(&mut fg, &mut bg);
            }

            if global_bg != bg {
                shapes.push(Shape::Rect(RectShape::filled(
                    Rect::from_min_size(
                        Pos2::new(x, y),
                        // + 1.0 is to fill grid border
                        Vec2::new(cell_width + 1., cell_height + 1.),
                    ),
                    CornerRadius::ZERO,
                    bg,
                )));
            }

            // Underline (SGR 4 / 4:2..4:5) and strikeout (SGR 9), drawn behind
            // the glyph. `underline_color` is the SGR-58 colour if the app set
            // one, else the cell's effective foreground.
            if underline_kind(flags).is_some() || flags.contains(cell::Flags::STRIKEOUT) {
                let deco = indexed
                    .cell
                    .underline_color()
                    .map(|c| resolve_color(&self.theme, colors, c))
                    .unwrap_or(fg);
                push_text_decoration(&mut shapes, flags, x, x + cell_width, y, cell_height, deco);
            }

            // Hovered-hyperlink underline — skipped when the cell already has a
            // real SGR underline, so a `\e[4m` link isn't underlined twice.
            if is_hovered_hyperling && !flags.intersects(cell::Flags::ALL_UNDERLINES) {
                let underline_height = y + cell_height;
                shapes.push(Shape::LineSegment {
                    points: [
                        Pos2::new(x, underline_height),
                        Pos2::new(x + cell_width, underline_height),
                    ],
                    stroke: Stroke::new(cell_height * 0.15, fg),
                });
            }

            // Box-drawing / block / shade / Powerline glyphs are drawn by
            // pomptty (crisp joins, exact scaling) rather than the font.
            let mut box_glyph_drawn = false;
            if indexed.c != ' ' && indexed.c != '\t' {
                if let Some(mut g) =
                    box_drawing::cell_glyph(indexed.c, x, y, cell_width, cell_height, fg)
                {
                    shapes.append(&mut g);
                    box_glyph_drawn = true;
                }
            }

            let is_cursor_cell = content.grid.cursor.point == indexed.point;
            if is_cursor_cell {
                cursor_target = Some((Pos2::new(x, y), cell_width));
            }

            // Draw text content
            if indexed.c != ' ' && indexed.c != '\t' && !box_glyph_drawn {
                if is_cursor_cell && is_app_cursor_mode {
                    std::mem::swap(&mut fg, &mut bg);
                }

                let font_id = self.font.font_id(is_bold, is_italic);
                let pos = Pos2 {
                    x: x + (cell_width / 2.0),
                    y,
                };
                if is_cursor_cell {
                    cursor_glyph = Some((indexed.c, font_id.clone(), pos));
                }
                shapes.push(painter.fonts_mut(|c| {
                    Shape::text(c, pos, Align2::CENTER_TOP, indexed.c, font_id, fg)
                }));
            }
        }

        if let Some((target, cell_w)) = cursor_target {
            let cursor_color = resolve_color(&self.theme, colors, Color::Named(NamedColor::Cursor));
            let bg_color = resolve_color(&self.theme, colors, Color::Named(NamedColor::Background));
            paint_cursor(
                cursor_color,
                bg_color,
                self.widget_id,
                self.has_focus,
                layout,
                painter,
                &mut shapes,
                content.cursor_style.shape,
                content.cursor_style.blinking,
                target,
                cell_w,
                cell_height,
                cursor_glyph,
            );
        }

        painter.extend(shapes);
    }
}

/// The configured theme's RGB for a `Colors` slot index (0..256 = the ANSI
/// palette, 256/257/258 = fg/bg/cursor, 259..269 = dim/bright variants).
/// Used to answer an OSC 4/10/11/12 colour *query* for a slot the app has
/// not overridden at runtime.
pub fn theme_rgb(theme: &TerminalTheme, index: usize) -> Rgb {
    let color = match index {
        0..=255 => Color::Indexed(index as u8),
        256 => Color::Named(NamedColor::Foreground),
        257 => Color::Named(NamedColor::Background),
        258 => Color::Named(NamedColor::Cursor),
        259 => Color::Named(NamedColor::DimBlack),
        260 => Color::Named(NamedColor::DimRed),
        261 => Color::Named(NamedColor::DimGreen),
        262 => Color::Named(NamedColor::DimYellow),
        263 => Color::Named(NamedColor::DimBlue),
        264 => Color::Named(NamedColor::DimMagenta),
        265 => Color::Named(NamedColor::DimCyan),
        266 => Color::Named(NamedColor::DimWhite),
        267 => Color::Named(NamedColor::BrightForeground),
        _ => Color::Named(NamedColor::DimForeground),
    };
    let c = theme.get_color(color);
    Rgb {
        r: c.r(),
        g: c.g(),
        b: c.b(),
    }
}

/// Which underline style a cell's flags select. A more specific style wins if
/// several bits are somehow set at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnderlineKind {
    Single,
    Double,
    Curl,
    Dotted,
    Dashed,
}

fn underline_kind(flags: cell::Flags) -> Option<UnderlineKind> {
    use cell::Flags as F;
    if flags.contains(F::DOUBLE_UNDERLINE) {
        Some(UnderlineKind::Double)
    } else if flags.contains(F::UNDERCURL) {
        Some(UnderlineKind::Curl)
    } else if flags.contains(F::DOTTED_UNDERLINE) {
        Some(UnderlineKind::Dotted)
    } else if flags.contains(F::DASHED_UNDERLINE) {
        Some(UnderlineKind::Dashed)
    } else if flags.contains(F::UNDERLINE) {
        Some(UnderlineKind::Single)
    } else {
        None
    }
}

/// Append the underline (SGR 4 / 4:2 / 4:3 / 4:4 / 4:5) and strikeout (SGR 9)
/// shapes for one cell. `color` is the SGR-58 underline colour if set, else
/// the cell's effective foreground.
fn push_text_decoration(
    shapes: &mut Vec<Shape>,
    flags: cell::Flags,
    x0: f32,
    x1: f32,
    y_top: f32,
    cell_height: f32,
    color: Color32,
) {
    let thickness = (cell_height * 0.07).clamp(1.0, 3.0);
    let uy = y_top + cell_height - thickness * 2.0;
    let seg = |shapes: &mut Vec<Shape>, y: f32, t: f32| {
        shapes.push(Shape::LineSegment {
            points: [Pos2::new(x0, y), Pos2::new(x1, y)],
            stroke: Stroke::new(t, color),
        });
    };

    match underline_kind(flags) {
        Some(UnderlineKind::Single) => seg(shapes, uy, thickness),
        Some(UnderlineKind::Double) => {
            let g = (thickness * 0.9).max(1.0);
            seg(shapes, uy - g, thickness * 0.75);
            seg(shapes, uy + g, thickness * 0.75);
        }
        Some(UnderlineKind::Curl) => shapes.push(undercurl(x0, x1, uy, thickness, color)),
        Some(UnderlineKind::Dotted) => shapes.extend(Shape::dashed_line(
            &[Pos2::new(x0, uy), Pos2::new(x1, uy)],
            Stroke::new(thickness, color),
            thickness,
            thickness * 1.5,
        )),
        Some(UnderlineKind::Dashed) => shapes.extend(Shape::dashed_line(
            &[Pos2::new(x0, uy), Pos2::new(x1, uy)],
            Stroke::new(thickness, color),
            thickness * 3.0,
            thickness * 2.5,
        )),
        None => {}
    }

    if flags.contains(cell::Flags::STRIKEOUT) {
        seg(shapes, y_top + cell_height * 0.52, thickness);
    }
}

/// A sine-wave undercurl across `x0..x1`. The phase is tied to the absolute
/// `x` so the wave of one cell joins up with its neighbours' rather than
/// restarting at every cell boundary.
fn undercurl(x0: f32, x1: f32, y: f32, thickness: f32, color: Color32) -> Shape {
    let wavelength = (thickness * 3.0).max(6.0);
    let amp = thickness;
    let step = 1.5_f32;
    let mut points = Vec::with_capacity(((x1 - x0) / step) as usize + 2);
    let mut x = x0;
    while x < x1 {
        let phase = x / wavelength * std::f32::consts::TAU;
        points.push(Pos2::new(x, y + phase.sin() * amp));
        x += step;
    }
    let phase = x1 / wavelength * std::f32::consts::TAU;
    points.push(Pos2::new(x1, y + phase.sin() * amp));
    Shape::line(points, Stroke::new(thickness, color))
}

/// Map one of the 8 normal palette colours to its bright counterpart, for the
/// `bold_is_bright` option. Anything else (256-colour, truecolour, the named
/// fg/bg/cursor slots) is returned unchanged.
fn brighten(c: Color) -> Color {
    use NamedColor as N;
    match c {
        Color::Named(n) => Color::Named(match n {
            N::Black => N::BrightBlack,
            N::Red => N::BrightRed,
            N::Green => N::BrightGreen,
            N::Yellow => N::BrightYellow,
            N::Blue => N::BrightBlue,
            N::Magenta => N::BrightMagenta,
            N::Cyan => N::BrightCyan,
            N::White => N::BrightWhite,
            other => other,
        }),
        Color::Indexed(i) if i < 8 => Color::Indexed(i + 8),
        other => other,
    }
}

/// Resolve an ANSI color to pixels: a live OSC 4/10/11/12 override if the app
/// set one, otherwise the configured theme.
fn resolve_color(theme: &TerminalTheme, overrides: &Colors, c: Color) -> Color32 {
    let over = match c {
        Color::Named(nc) => overrides[nc],
        Color::Indexed(i) => overrides[i as usize],
        Color::Spec(_) => None,
    };
    match over {
        Some(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
        None => theme.get_color(c),
    }
}

/// Draw the cursor at (an eased approach to) `target`, honouring its real
/// shape/blink from `RenderableContent::cursor_style`. Unfocused shows a
/// steady hollow outline instead of a blinking fill. A solid `Block` redraws
/// the character underneath in `bg_color` on top, so it stays legible
/// regardless of the cursor color. `color`/`bg_color` are already resolved
/// (theme + any OSC 12 / OSC 11 override).
#[allow(clippy::too_many_arguments)]
fn paint_cursor(
    color: Color32,
    bg_color: Color32,
    widget_id: Id,
    has_focus: bool,
    layout: &Response,
    painter: &Painter,
    shapes: &mut Vec<Shape>,
    shape: CursorShape,
    blinking: bool,
    target: Pos2,
    cell_width: f32,
    cell_height: f32,
    glyph: Option<(char, FontId, Pos2)>,
) {
    if shape == CursorShape::Hidden {
        return;
    }

    let ctx = &layout.ctx;
    let x = ctx.animate_value_with_time(widget_id.with("cursor_x"), target.x, 0.08);
    let y = ctx.animate_value_with_time(widget_id.with("cursor_y"), target.y, 0.08);
    let pos = Pos2::new(x, y);

    let outline_only = shape == CursorShape::HollowBlock || !has_focus;

    // Blink as an eased fade rather than a hard on/off. A window-compositor
    // that tears the window blit (Marco's built-in one, when the window isn't
    // maximised) then shows the cursor at two near-equal opacities top and
    // bottom instead of a half-drawn "striped" block.
    let blink = if has_focus && blinking {
        let period = 0.53_f64;
        let time = ctx.input(|i| i.time);
        let on = (time / period) as i64 % 2 == 0;
        // Wake up at the next toggle; `animate_bool_with_time` drives the ease.
        ctx.request_repaint_after(Duration::from_secs_f64(period - time.rem_euclid(period)));
        ctx.animate_bool_with_time(widget_id.with("cursor_blink"), on, 0.16)
    } else {
        1.0
    };
    if blink <= 0.02 {
        return;
    }
    let fade = |c: Color32| {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * blink).round() as u8)
    };

    let rect = match shape {
        CursorShape::Hidden => unreachable!(),
        CursorShape::Block | CursorShape::HollowBlock => {
            Rect::from_min_size(pos, Vec2::new(cell_width, cell_height))
        }
        CursorShape::Underline => {
            let t = (cell_height * 0.15).max(1.5);
            Rect::from_min_size(
                Pos2::new(pos.x, pos.y + cell_height - t),
                Vec2::new(cell_width, t),
            )
        }
        CursorShape::Beam => {
            let t = (cell_width * 0.12).max(1.5);
            Rect::from_min_size(pos, Vec2::new(t, cell_height))
        }
    };

    // Snap the cursor's edges to whole physical pixels. The grid origin often
    // lands on a fractional pixel (the window isn't an exact multiple of the
    // cell size), and a filled/stroked rect straddling pixel rows renders with
    // half-covered rows top and bottom — which reads as faint stripes across
    // the cursor, especially when the window isn't maximized.
    let ppp = ctx.pixels_per_point().max(0.01);
    let snap = |v: f32| (v * ppp).round() / ppp;
    let rect = Rect::from_min_max(
        Pos2::new(snap(rect.min.x), snap(rect.min.y)),
        Pos2::new(snap(rect.max.x), snap(rect.max.y)),
    );

    if outline_only {
        shapes.push(Shape::Rect(RectShape::stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, fade(color)),
            StrokeKind::Inside,
        )));
    } else {
        shapes.push(Shape::Rect(RectShape::filled(
            rect,
            CornerRadius::ZERO,
            fade(color),
        )));
        if shape == CursorShape::Block {
            if let Some((c, font_id, glyph_pos)) = glyph {
                // Sit the punched-through glyph exactly on the snapped block
                // so no sliver of cursor colour shows above or below it; fade
                // it with the block so the real glyph shows as it blinks out.
                let glyph_pos = Pos2::new(glyph_pos.x, rect.min.y);
                shapes.push(painter.fonts_mut(|f| {
                    Shape::text(f, glyph_pos, Align2::CENTER_TOP, c, font_id, fade(bg_color))
                }));
            }
        }
    }
}

fn process_keyboard_event(
    event: egui::Event,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    modifiers: Modifiers,
) -> InputAction {
    match event {
        egui::Event::Text(text) => process_text_event(&text, modifiers, backend, bindings_layout),
        egui::Event::Paste(text) => InputAction::BackendCall(
            #[cfg(not(any(target_os = "ios", target_os = "macos")))]
            if modifiers.contains(Modifiers::COMMAND | Modifiers::SHIFT) {
                BackendCommand::Paste(text)
            } else {
                // Hotfix - Send ^V when there's not selection on view.
                BackendCommand::Write([0x16].to_vec())
            },
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            BackendCommand::Paste(text),
        ),
        egui::Event::Copy => {
            #[cfg(not(any(target_os = "ios", target_os = "macos")))]
            if modifiers.contains(Modifiers::COMMAND | Modifiers::SHIFT) {
                let content = backend.selectable_content();
                InputAction::WriteToClipboard(content)
            } else {
                // Hotfix - Send ^C when there's not selection on view.
                InputAction::BackendCall(BackendCommand::Write([0x3].to_vec()))
            }
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            {
                let content = backend.selectable_content();
                InputAction::WriteToClipboard(content)
            }
        }
        egui::Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } => process_keyboard_key(backend, bindings_layout, key, modifiers, pressed),
        _ => InputAction::Ignore,
    }
}

fn process_text_event(
    text: &str,
    modifiers: Modifiers,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
) -> InputAction {
    if let Some(key) = Key::from_name(text) {
        if bindings_layout.get_action(
            InputKind::KeyCode(key),
            modifiers,
            backend.last_content().terminal_mode,
        ) == BindingAction::Ignore
        {
            InputAction::BackendCall(BackendCommand::Write(text.as_bytes().to_vec()))
        } else {
            InputAction::Ignore
        }
    } else {
        InputAction::BackendCall(BackendCommand::Write(text.as_bytes().to_vec()))
    }
}

fn process_keyboard_key(
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    key: Key,
    modifiers: Modifiers,
    pressed: bool,
) -> InputAction {
    if !pressed {
        return InputAction::Ignore;
    }

    let terminal_mode = backend.last_content().terminal_mode;
    let binding_action =
        bindings_layout.get_action(InputKind::KeyCode(key), modifiers, terminal_mode);

    match binding_action {
        BindingAction::Char(c) => {
            let mut buf = [0, 0, 0, 0];
            let str = c.encode_utf8(&mut buf);
            InputAction::BackendCall(BackendCommand::Write(str.as_bytes().to_vec()))
        }
        BindingAction::Esc(seq) => {
            InputAction::BackendCall(BackendCommand::Write(seq.as_bytes().to_vec()))
        }
        _ => InputAction::Ignore,
    }
}

fn process_mouse_wheel(
    state: &mut TerminalViewState,
    font_size: f32,
    unit: MouseWheelUnit,
    delta: Vec2,
) -> InputAction {
    match unit {
        MouseWheelUnit::Line => {
            let lines = delta.y.signum() * delta.y.abs().ceil();
            InputAction::BackendCall(BackendCommand::Scroll(lines as i32))
        }
        MouseWheelUnit::Point => {
            state.scroll_pixels -= delta.y;
            let lines = (state.scroll_pixels / font_size).trunc();
            state.scroll_pixels %= font_size;
            if lines != 0.0 {
                InputAction::BackendCall(BackendCommand::Scroll(-lines as i32))
            } else {
                InputAction::Ignore
            }
        }
        MouseWheelUnit::Page => InputAction::Ignore,
    }
}

#[allow(clippy::too_many_arguments)]
fn process_button_click(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    button: PointerButton,
    position: Pos2,
    modifiers: &Modifiers,
    pressed: bool,
) -> InputAction {
    match button {
        PointerButton::Primary => process_left_button(
            state,
            layout,
            backend,
            bindings_layout,
            position,
            modifiers,
            pressed,
        ),
        _ => InputAction::Ignore,
    }
}

fn process_left_button(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    position: Pos2,
    modifiers: &Modifiers,
    pressed: bool,
) -> InputAction {
    let terminal_mode = backend.last_content().terminal_mode;
    if terminal_mode.intersects(TermMode::MOUSE_MODE) {
        InputAction::BackendCall(BackendCommand::MouseReport(
            MouseButton::LeftButton,
            *modifiers,
            state.current_mouse_position_on_grid,
            pressed,
        ))
    } else if pressed {
        process_left_button_pressed(state, layout, position, modifiers)
    } else {
        process_left_button_released(state, layout, backend, bindings_layout, position, modifiers)
    }
}

fn process_left_button_pressed(
    state: &mut TerminalViewState,
    layout: &Response,
    position: Pos2,
    modifiers: &Modifiers,
) -> InputAction {
    state.is_dragged = true;
    InputAction::BackendCall(build_start_select_command(layout, position, modifiers))
}

fn process_left_button_released(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    position: Pos2,
    modifiers: &Modifiers,
) -> InputAction {
    state.is_dragged = false;
    if layout.double_clicked() || layout.triple_clicked() {
        InputAction::BackendCall(build_start_select_command(layout, position, modifiers))
    } else {
        let terminal_content = backend.last_content();
        let binding_action = bindings_layout.get_action(
            InputKind::Mouse(PointerButton::Primary),
            *modifiers,
            terminal_content.terminal_mode,
        );

        if binding_action == BindingAction::LinkOpen {
            InputAction::BackendCall(BackendCommand::ProcessLink(
                LinkAction::Open,
                state.current_mouse_position_on_grid,
            ))
        } else {
            InputAction::Ignore
        }
    }
}

fn build_start_select_command(
    layout: &Response,
    cursor_position: Pos2,
    modifiers: &Modifiers,
) -> BackendCommand {
    let selection_type = if layout.double_clicked() {
        SelectionType::Semantic
    } else if layout.triple_clicked() {
        SelectionType::Lines
    } else if modifiers.alt && modifiers.command {
        // Ctrl+Alt+drag: rectangular / column selection. (Plain Alt+drag is
        // commonly the window manager's move-window gesture on X11.)
        SelectionType::Block
    } else {
        SelectionType::Simple
    };

    BackendCommand::SelectStart(
        selection_type,
        cursor_position.x - layout.rect.min.x,
        cursor_position.y - layout.rect.min.y,
    )
}

fn process_mouse_move(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    position: Pos2,
    modifiers: &Modifiers,
) -> Vec<InputAction> {
    let terminal_content = backend.last_content();
    let cursor_x = position.x - layout.rect.min.x;
    let cursor_y = position.y - layout.rect.min.y;
    state.current_mouse_position_on_grid = TerminalBackend::selection_point(
        cursor_x,
        cursor_y,
        &terminal_content.terminal_size,
        terminal_content.grid.display_offset(),
    );

    let mut actions = vec![];
    // Handle command or selection update based on terminal mode and modifiers
    if state.is_dragged {
        let terminal_mode = terminal_content.terminal_mode;
        let cmd = if terminal_mode.contains(TermMode::MOUSE_MOTION) && modifiers.is_none() {
            InputAction::BackendCall(BackendCommand::MouseReport(
                MouseButton::LeftMove,
                *modifiers,
                state.current_mouse_position_on_grid,
                true,
            ))
        } else {
            InputAction::BackendCall(BackendCommand::SelectUpdate(cursor_x, cursor_y))
        };

        actions.push(cmd);
    }

    // Handle link hover if applicable
    if modifiers.command_only() {
        actions.push(InputAction::BackendCall(BackendCommand::ProcessLink(
            LinkAction::Hover,
            state.current_mouse_position_on_grid,
        )));
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::{brighten, theme_rgb, underline_kind, UnderlineKind};
    use crate::theme::TerminalTheme;
    use alacritty_terminal::term::cell::Flags;
    use alacritty_terminal::vte::ansi::{Color, NamedColor};

    #[test]
    fn brighten_maps_only_the_normal_eight() {
        assert_eq!(
            brighten(Color::Named(NamedColor::Red)),
            Color::Named(NamedColor::BrightRed)
        );
        assert_eq!(brighten(Color::Indexed(2)), Color::Indexed(10));
        assert_eq!(brighten(Color::Indexed(200)), Color::Indexed(200));
        assert_eq!(
            brighten(Color::Named(NamedColor::Foreground)),
            Color::Named(NamedColor::Foreground)
        );
    }

    #[test]
    fn underline_kind_picks_the_right_style() {
        assert_eq!(underline_kind(Flags::empty()), None);
        assert_eq!(
            underline_kind(Flags::UNDERLINE),
            Some(UnderlineKind::Single)
        );
        assert_eq!(underline_kind(Flags::UNDERCURL), Some(UnderlineKind::Curl));
        assert_eq!(
            underline_kind(Flags::DOTTED_UNDERLINE),
            Some(UnderlineKind::Dotted)
        );
        // A more specific style wins over plain UNDERLINE.
        assert_eq!(
            underline_kind(Flags::UNDERLINE | Flags::UNDERCURL),
            Some(UnderlineKind::Curl)
        );
        assert_eq!(
            underline_kind(Flags::DOUBLE_UNDERLINE | Flags::UNDERCURL),
            Some(UnderlineKind::Double)
        );
        // STRIKEOUT alone is not an underline.
        assert_eq!(underline_kind(Flags::STRIKEOUT), None);
    }

    #[test]
    fn theme_rgb_maps_named_and_indexed_slots() {
        let theme = TerminalTheme::default();
        // 256/257/258 = foreground / background / cursor.
        let fg = theme_rgb(&theme, 256);
        assert_eq!((fg.r, fg.g, fg.b), (0xd8, 0xd8, 0xd8));
        let bg = theme_rgb(&theme, 257);
        assert_eq!((bg.r, bg.g, bg.b), (0x18, 0x18, 0x18));
        // Palette index 1 = red.
        let red = theme_rgb(&theme, 1);
        assert_eq!((red.r, red.g, red.b), (0xac, 0x42, 0x42));
    }
}
