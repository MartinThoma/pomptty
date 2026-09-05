//! Box-drawing (`U+2500`–`U+257F`), block/shade elements (`U+2580`–`U+259F`)
//! and Powerline separators (`U+E0B0`–`U+E0B3`) drawn by pomptty itself
//! rather than the fallback font: lines join across cells with no sub-pixel
//! gap, everything scales exactly with the cell, and powerline prompts work
//! without a patched font.
//!
//! [`cell_glyph`] returns `Some(shapes)` for a codepoint we draw, or `None`
//! to fall through to the normal font path.

use egui::epaint::RectShape;
use egui::{Color32, CornerRadius, Pos2, Rect, Shape, Stroke, Vec2};

/// The shapes for one cell's glyph, or `None` to let the font draw it.
/// `color` is the cell's already-resolved foreground.
pub fn cell_glyph(c: char, x: f32, y: f32, w: f32, h: f32, color: Color32) -> Option<Vec<Shape>> {
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    match c as u32 {
        0x2571..=0x2573 => Some(diagonals(c, r, color)),
        0x2500..=0x254B | 0x2574..=0x257F => line_arms(c, r, color),
        0x2550..=0x256C => double_arms(c, r, color),
        0x2580..=0x259F => Some(blocks(c, r, color)),
        0xE0B0..=0xE0B3 => Some(powerline(c, r, color)),
        _ => None,
    }
}

fn fill(x0: f32, y0: f32, x1: f32, y1: f32, color: Color32) -> Shape {
    Shape::Rect(RectShape::filled(
        Rect::from_min_max(
            Pos2::new(x0.round(), y0.round()),
            Pos2::new(x1.round(), y1.round()),
        ),
        CornerRadius::ZERO,
        color,
    ))
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).round() as u8)
}

// ---- line box-drawing (single: light / heavy) --------------------------------

/// `[up, right, down, left]`, each `0` none / `1` light / `2` heavy. `[0;4]`
/// means "not handled here" (dashed variants, rounded corners) — fall through.
fn line_table(c: char) -> Option<[u8; 4]> {
    let n = c as u32;
    #[rustfmt::skip]
    const T: [[u8; 4]; 0x4C] = [
        /*2500 ─*/[0,1,0,1], /*2501 ━*/[0,2,0,2], /*2502 │*/[1,0,1,0], /*2503 ┃*/[2,0,2,0],
        /*2504*/[0,0,0,0], /*2505*/[0,0,0,0], /*2506*/[0,0,0,0], /*2507*/[0,0,0,0],
        /*2508*/[0,0,0,0], /*2509*/[0,0,0,0], /*250A*/[0,0,0,0], /*250B*/[0,0,0,0],
        /*250C ┌*/[0,1,1,0], /*250D ┍*/[0,2,1,0], /*250E ┎*/[0,1,2,0], /*250F ┏*/[0,2,2,0],
        /*2510 ┐*/[0,0,1,1], /*2511 ┑*/[0,0,1,2], /*2512 ┒*/[0,0,2,1], /*2513 ┓*/[0,0,2,2],
        /*2514 └*/[1,1,0,0], /*2515 ┕*/[1,2,0,0], /*2516 ┖*/[2,1,0,0], /*2517 ┗*/[2,2,0,0],
        /*2518 ┘*/[1,0,0,1], /*2519 ┙*/[1,0,0,2], /*251A ┚*/[2,0,0,1], /*251B ┛*/[2,0,0,2],
        /*251C ├*/[1,1,1,0], /*251D ┝*/[1,2,1,0], /*251E ┞*/[2,1,1,0], /*251F ┟*/[1,1,2,0],
        /*2520 ┠*/[2,1,2,0], /*2521 ┡*/[2,2,1,0], /*2522 ┢*/[1,2,2,0], /*2523 ┣*/[2,2,2,0],
        /*2524 ┤*/[1,0,1,1], /*2525 ┥*/[1,0,1,2], /*2526 ┦*/[2,0,1,1], /*2527 ┧*/[1,0,2,1],
        /*2528 ┨*/[2,0,2,1], /*2529 ┩*/[2,0,1,2], /*252A ┪*/[1,0,2,2], /*252B ┫*/[2,0,2,2],
        /*252C ┬*/[0,1,1,1], /*252D ┭*/[0,1,1,2], /*252E ┮*/[0,2,1,1], /*252F ┯*/[0,2,1,2],
        /*2530 ┰*/[0,1,2,1], /*2531 ┱*/[0,1,2,2], /*2532 ┲*/[0,2,2,1], /*2533 ┳*/[0,2,2,2],
        /*2534 ┴*/[1,1,0,1], /*2535 ┵*/[1,1,0,2], /*2536 ┶*/[1,2,0,1], /*2537 ┷*/[1,2,0,2],
        /*2538 ┸*/[2,1,0,1], /*2539 ┹*/[2,1,0,2], /*253A ┺*/[2,2,0,1], /*253B ┻*/[2,2,0,2],
        /*253C ┼*/[1,1,1,1], /*253D ┽*/[1,1,1,2], /*253E ┾*/[1,2,1,1], /*253F ┿*/[1,2,1,2],
        /*2540 ╀*/[2,1,1,1], /*2541 ╁*/[1,1,2,1], /*2542 ╂*/[2,1,2,1], /*2543 ╃*/[2,1,1,2],
        /*2544 ╄*/[2,2,1,1], /*2545 ╅*/[1,1,2,2], /*2546 ╆*/[1,2,2,1], /*2547 ╇*/[2,2,1,2],
        /*2548 ╈*/[1,2,2,2], /*2549 ╉*/[2,1,2,2], /*254A ╊*/[2,2,2,1], /*254B ╋*/[2,2,2,2],
    ];
    if (0x2500..=0x254B).contains(&n) {
        let a = T[(n - 0x2500) as usize];
        return (a != [0, 0, 0, 0]).then_some(a);
    }
    #[rustfmt::skip]
    let partial = match n {
        0x2574 => [0,0,0,1], 0x2575 => [1,0,0,0], 0x2576 => [0,1,0,0], 0x2577 => [0,0,1,0],
        0x2578 => [0,0,0,2], 0x2579 => [2,0,0,0], 0x257A => [0,2,0,0], 0x257B => [0,0,2,0],
        0x257C => [0,2,0,1], 0x257D => [1,0,2,0], 0x257E => [0,1,0,2], 0x257F => [2,0,1,0],
        _ => return None,
    };
    Some(partial)
}

fn line_arms(c: char, r: Rect, color: Color32) -> Option<Vec<Shape>> {
    let arms = line_table(c)?;
    let light = (r.height() * 0.09).round().max(1.0);
    let heavy = (r.height() * 0.17).round().max(2.0);
    let th = |w: u8| if w == 2 { heavy } else { light };
    let cx = ((r.left() + r.right()) / 2.0).round();
    let cy = ((r.top() + r.bottom()) / 2.0).round();
    let over = arms
        .iter()
        .filter(|&&w| w > 0)
        .map(|&w| th(w))
        .fold(0.0_f32, f32::max)
        / 2.0;

    let mut out = Vec::new();
    if arms[0] > 0 {
        let t = th(arms[0]);
        out.push(fill(cx - t / 2.0, r.top(), cx + t / 2.0, cy + over, color));
    }
    if arms[2] > 0 {
        let t = th(arms[2]);
        out.push(fill(
            cx - t / 2.0,
            cy - over,
            cx + t / 2.0,
            r.bottom(),
            color,
        ));
    }
    if arms[3] > 0 {
        let t = th(arms[3]);
        out.push(fill(r.left(), cy - t / 2.0, cx + over, cy + t / 2.0, color));
    }
    if arms[1] > 0 {
        let t = th(arms[1]);
        out.push(fill(
            cx - over,
            cy - t / 2.0,
            r.right(),
            cy + t / 2.0,
            color,
        ));
    }
    Some(out)
}

// ---- double-line box-drawing ------------------------------------------------

/// `[up, right, down, left]`, `0` none / `1` single / `2` double.
fn double_table(c: char) -> Option<[u8; 4]> {
    #[rustfmt::skip]
    let a = match c as u32 {
        0x2550 => [0,2,0,2], 0x2551 => [2,0,2,0],
        0x2552 => [0,2,1,0], 0x2553 => [0,1,2,0], 0x2554 => [0,2,2,0],
        0x2555 => [0,0,1,2], 0x2556 => [0,0,2,1], 0x2557 => [0,0,2,2],
        0x2558 => [1,2,0,0], 0x2559 => [2,1,0,0], 0x255A => [2,2,0,0],
        0x255B => [1,0,0,2], 0x255C => [2,0,0,1], 0x255D => [2,0,0,2],
        0x255E => [1,2,1,0], 0x255F => [2,1,2,0], 0x2560 => [2,2,2,0],
        0x2561 => [1,0,1,2], 0x2562 => [2,0,2,1], 0x2563 => [2,0,2,2],
        0x2564 => [0,2,1,2], 0x2565 => [0,1,2,1], 0x2566 => [0,2,2,2],
        0x2567 => [1,2,0,2], 0x2568 => [2,1,0,1], 0x2569 => [2,2,0,2],
        0x256A => [1,2,1,2], 0x256B => [2,1,2,1], 0x256C => [2,2,2,2],
        _ => return None,
    };
    Some(a)
}

fn double_arms(c: char, r: Rect, color: Color32) -> Option<Vec<Shape>> {
    let arms = double_table(c)?;
    let t = (r.height() * 0.09).round().max(1.0);
    let gap = (t + 1.0).max(2.0);
    let cx = ((r.left() + r.right()) / 2.0).round();
    let cy = ((r.top() + r.bottom()) / 2.0).round();
    let mut out = Vec::new();

    // vertical rails
    for (dir, y0, y1) in [(0u8, r.top(), cy + gap), (2, cy - gap, r.bottom())] {
        match arms[dir as usize] {
            1 => out.push(fill(cx - t / 2.0, y0, cx + t / 2.0, y1, color)),
            2 => {
                out.push(fill(cx - gap - t / 2.0, y0, cx - gap + t / 2.0, y1, color));
                out.push(fill(cx + gap - t / 2.0, y0, cx + gap + t / 2.0, y1, color));
            }
            _ => {}
        }
    }
    // horizontal rails
    for (dir, x0, x1) in [(3u8, r.left(), cx + gap), (1, cx - gap, r.right())] {
        match arms[dir as usize] {
            1 => out.push(fill(x0, cy - t / 2.0, x1, cy + t / 2.0, color)),
            2 => {
                out.push(fill(x0, cy - gap - t / 2.0, x1, cy - gap + t / 2.0, color));
                out.push(fill(x0, cy + gap - t / 2.0, x1, cy + gap + t / 2.0, color));
            }
            _ => {}
        }
    }
    Some(out)
}

// ---- block / shade / quadrant elements -------------------------------------

fn blocks(c: char, r: Rect, color: Color32) -> Vec<Shape> {
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let (w, h) = (r.width(), r.height());
    let mx = ((l + rt) / 2.0).round();
    let my = ((t + b) / 2.0).round();
    match c as u32 {
        0x2580 => vec![fill(l, t, rt, my, color)],
        0x2581..=0x2588 => {
            let k = (c as u32 - 0x2580) as f32 / 8.0;
            vec![fill(l, b - h * k, rt, b, color)]
        }
        0x2589..=0x258F => {
            let k = (0x2590 - c as u32) as f32 / 8.0;
            vec![fill(l, t, l + w * k, b, color)]
        }
        0x2590 => vec![fill(mx, t, rt, b, color)],
        0x2591 => vec![fill(l, t, rt, b, with_alpha(color, 0.25))],
        0x2592 => vec![fill(l, t, rt, b, with_alpha(color, 0.5))],
        0x2593 => vec![fill(l, t, rt, b, with_alpha(color, 0.75))],
        0x2594 => vec![fill(l, t, rt, t + h / 8.0, color)],
        0x2595 => vec![fill(rt - w / 8.0, t, rt, b, color)],
        0x2596..=0x259F => quadrants(c, r, color, mx, my),
        _ => vec![],
    }
}

fn quadrants(c: char, r: Rect, color: Color32, mx: f32, my: f32) -> Vec<Shape> {
    // bit 0 = upper-left, 1 = upper-right, 2 = lower-left, 3 = lower-right
    let mask: u8 = match c as u32 {
        0x2596 => 0b0100,
        0x2597 => 0b1000,
        0x2598 => 0b0001,
        0x2599 => 0b1101,
        0x259A => 0b1001,
        0x259B => 0b0111,
        0x259C => 0b1011,
        0x259D => 0b0010,
        0x259E => 0b0110,
        0x259F => 0b1110,
        _ => 0,
    };
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let quads = [
        (0b0001, l, t, mx, my),
        (0b0010, mx, t, rt, my),
        (0b0100, l, my, mx, b),
        (0b1000, mx, my, rt, b),
    ];
    quads
        .into_iter()
        .filter(|&(bit, ..)| mask & bit != 0)
        .map(|(_, x0, y0, x1, y1)| fill(x0, y0, x1, y1, color))
        .collect()
}

// ---- diagonals ------------------------------------------------------------

fn diagonals(c: char, r: Rect, color: Color32) -> Vec<Shape> {
    let t = (r.height() * 0.09).max(1.0);
    let seg = |a: Pos2, b: Pos2| Shape::LineSegment {
        points: [a, b],
        stroke: Stroke::new(t, color),
    };
    let mut out = Vec::new();
    if matches!(c as u32, 0x2571 | 0x2573) {
        out.push(seg(r.left_bottom(), r.right_top()));
    }
    if matches!(c as u32, 0x2572 | 0x2573) {
        out.push(seg(r.left_top(), r.right_bottom()));
    }
    out
}

// ---- Powerline separators ------------------------------------------------

fn powerline(c: char, r: Rect, color: Color32) -> Vec<Shape> {
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let my = (t + b) / 2.0;
    let t_stroke = (r.height() * 0.11).max(1.5);
    match c as u32 {
        0xE0B0 => vec![Shape::convex_polygon(
            vec![Pos2::new(l, t), Pos2::new(rt, my), Pos2::new(l, b)],
            color,
            Stroke::NONE,
        )],
        0xE0B2 => vec![Shape::convex_polygon(
            vec![Pos2::new(rt, t), Pos2::new(l, my), Pos2::new(rt, b)],
            color,
            Stroke::NONE,
        )],
        0xE0B1 => vec![Shape::line(
            vec![Pos2::new(l, t), Pos2::new(rt, my), Pos2::new(l, b)],
            Stroke::new(t_stroke, color),
        )],
        0xE0B3 => vec![Shape::line(
            vec![Pos2::new(rt, t), Pos2::new(l, my), Pos2::new(rt, b)],
            Stroke::new(t_stroke, color),
        )],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(c: char) -> bool {
        cell_glyph(c, 0.0, 0.0, 8.0, 16.0, Color32::WHITE).is_some()
    }

    #[test]
    fn handles_one_of_each_category() {
        assert!(some('─')); // light line
        assert!(some('┼')); // cross
        assert!(some('┏')); // heavy corner
        assert!(some('╬')); // double cross
        assert!(some('▄')); // lower block
        assert!(some('▌')); // left block
        assert!(some('░')); // shade
        assert!(some('▚')); // quadrant
        assert!(some('╱')); // diagonal
        assert!(some('\u{E0B0}')); // powerline
    }

    #[test]
    fn falls_through_for_text_and_dashes() {
        assert!(!some('A'));
        assert!(!some(' '));
        assert!(!some('┄')); // dashed — let the font handle it
        assert!(!some('╭')); // rounded corner — font
    }

    #[test]
    fn cross_has_four_light_arms() {
        assert_eq!(line_table('┼'), Some([1, 1, 1, 1]));
        assert_eq!(line_table('━'), Some([0, 2, 0, 2]));
        assert_eq!(line_table('┄'), None);
    }
}
