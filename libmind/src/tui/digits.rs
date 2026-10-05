//! Large digits on the cell grid (issue 089): `0`–`9`, `:` and `-` as 3 × 5 dot glyphs (`:` one dot wide), a dot
//! `scale` cells wide and `scale` half cells tall, two dots per cell vertically (`▀ ▄ █`) so that dots are square on
//! the 8 × 16 font. Clocks size them to whatever grid they get: a screen, a text window, a small one.
use super::{Grid, Rect, Style};

// Rows of 3 dots, the most significant of 3 bits on the left.
const GLYPHS: [[u8; 5]; 12] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b011, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
    [0b000, 0b010, 0b000, 0b010, 0b000], // :  (the middle column)
    [0b000, 0b000, 0b111, 0b000, 0b000], // -
];

fn glyph(ch: char) -> Option<(&'static [u8; 5], usize)> {
    match ch {
        '0'..='9' => Some((&GLYPHS[ch as usize - '0' as usize], 3)),
        ':' => Some((&GLYPHS[10], 1)),
        '-' => Some((&GLYPHS[11], 3)),
        _ => None,
    }
}

// Whether dot (column, row) of `ch` is lit; `:` keeps the middle column of its 3-dot pattern.
fn lit(ch: char, column: usize, row: usize) -> bool {
    let Some((rows, width)) = glyph(ch) else { return false };
    let bit = if width == 1 { 1 } else { 2 - column };
    rows[row] >> bit & 1 != 0
}

/// Cells `text` takes at `scale` (one dot of space between characters).
pub fn size(text: &str, scale: usize) -> (usize, usize) {
    let dots: usize = text.chars().filter_map(glyph).map(|(_, w)| w).sum::<usize>() + text.chars().count().saturating_sub(1);
    (dots * scale, (5 * scale).div_ceil(2))
}

/// The largest scale (up to `max`) at which `text` fits `w` × `h` cells; None if not even 1 does.
pub fn fit(text: &str, w: usize, h: usize, max: usize) -> Option<usize> {
    (1..=max.max(1)).rev().find(|&scale| { let (tw, th) = size(text, scale); tw <= w && th <= h })
}

/// Draws `text` with its top left corner at (x, y); returns the area it took.
pub fn draw(grid: &mut Grid, x: usize, y: usize, text: &str, scale: usize, style: Style) -> Rect {
    let scale = scale.max(1);
    let (w, h) = size(text, scale);
    // Which character and dot column each cell column shows.
    let mut columns = [(' ', 0usize); 512];
    let mut at = 0;
    for (i, ch) in text.chars().enumerate() {
        let Some((_, width)) = glyph(ch) else { continue };
        if i > 0 { at += scale; } // the space between characters
        for dot in 0..width { for _ in 0..scale { if at < columns.len() { columns[at] = (ch, dot); } at += 1; } }
    }
    for row in 0..h {
        for column in 0..w.min(columns.len()) {
            let (ch, dot) = columns[column];
            let on = |half: usize| half < 5 * scale && ch != ' ' && lit(ch, dot, half / scale);
            let (top, bottom) = (on(2 * row), on(2 * row + 1));
            let cell = match (top, bottom) { (true, true) => '█', (true, false) => '▀', (false, true) => '▄', _ => ' ' };
            if cell != ' ' || (top, bottom) == (false, false) { grid.put(x + column, y + row, cell, style); }
        }
    }
    Rect::new(x, y, w, h)
}
