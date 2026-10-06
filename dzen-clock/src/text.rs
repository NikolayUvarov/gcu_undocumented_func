//! The text face of `dzen-clock` (issue 089): the five indicators as discs of colored cells, the orbit, its ticks and
//! the moving dot in characters, the digital time and the hints, sized to the grid (a screen, or a text window of
//! `wm` that may be resized). What is shown comes from `face.rs` and `cycle.rs`, as for the pixel face. Cells are one
//! unit wide and two tall, so the discs and the orbit stay round. No system calls: tests/clock_host.rs.
use crate::cycle::{point_at, OrbitMode};
use crate::face::{time_text, Face, OFF};
use crate::tui::{Grid, Style};

// The pixel face's colors.
const RIM: u32 = 0x1C2632;
const DIGITS: u32 = 0x697583;
const HINT: u32 = 0x394553;
const CYCLE: u32 = 0x808080;
const ORBIT_SIMPLE: u32 = 0x404850;
const ORBIT_TICKS: u32 = 0x505860;
const START_TICK: u32 = 0x909090;
const SMALL_TICK: u32 = 0x707070;

pub const TITLE: &str = "DZEN CLOCK";

/// The face on one line (`dzen-clock --line`, issue u016): the time, then the discs as the letters of their colors —
/// top left and top right, the center, bottom left and bottom right; W a white step, `·` a disc that is off. Returns
/// the bytes and their number.
pub fn line(face: Face, seconds: usize) -> ([u8; 32], usize) {
    let letter = |color: u32| -> &'static str {
        match crate::face::COLORS.iter().position(|&c| c == color) {
            Some(i) => ["R", "Y", "G", "C", "B", "M"][i],
            None if color == crate::face::WHITE => "W",
            None => "·",
        }
    };
    let mut out = [0u8; 32];
    let mut n = 0;
    let time = time_text(seconds);
    let parts = [core::str::from_utf8(&time).unwrap_or(""), "  ", letter(face.corners[3]), letter(face.corners[0]), " ", letter(face.center), " ",
                 letter(face.corners[2]), letter(face.corners[1])];
    for part in parts {
        out[n..n + part.len()].copy_from_slice(part.as_bytes());
        n += part.len();
    }
    (out, n)
}
pub const KEYS: &str = "D: DIGITS   C: ORBIT   P: 10S TICKS   H: HINTS   ESC: EXIT";
/// On the program's own screen T switches to the pixel face.
pub const KEYS_SWITCH: &str = "D: DIGITS   C: ORBIT   P: 10S TICKS   H: HINTS   T: PIXEL FACE   ESC: EXIT";

/// What the keys chose; `switch`: the key line names T (the program has a screen of its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Show { pub digits: bool, pub hints: bool, pub mode: OrbitMode, pub switch: bool }

/// Where the face is: its center in units (x: cells, y: half cells) and the distance of the corner indicators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout { pub x: isize, pub y: isize, pub half: isize, pub radius: isize }

/// The face fills the rows between the title and the digital time.
pub fn layout(cols: usize, rows: usize) -> Layout {
    let region = rows.saturating_sub(3).max(1); // rows 1 .. rows - 2
    let side = cols.min(2 * region) as isize;
    let half = (side * 3 / 10).max(3);
    Layout { x: cols as isize / 2, y: 2 + region as isize, half, radius: (half * 2 / 5).max(1) }
}

/// The cell that shows the point (x, y) units from the center, if inside the grid.
fn cell(l: &Layout, (x, y): (isize, isize), cols: usize, rows: usize) -> Option<(usize, usize)> {
    let (cx, cy) = (l.x + x, (l.y + y).div_euclid(2));
    (cx >= 0 && cy >= 0 && (cx as usize) < cols && (cy as usize) < rows).then_some((cx as usize, cy as usize))
}

/// Draws the face of `face`, with the orbit's dot at `phase` (ms into the 100 s turn) when the orbit is shown.
pub fn draw(grid: &mut Grid, face: Face, seconds: Option<usize>, phase: Option<usize>, show: Show) {
    let (cols, rows) = (grid.cols, grid.rows);
    let background = Style::new(OFF, OFF);
    grid.clear(background);
    let l = layout(cols, rows);
    // The indicators: corners clockwise from the top right, then the center.
    let discs = [((l.half, -l.half), face.corners[0]), ((l.half, l.half), face.corners[1]), ((-l.half, l.half), face.corners[2]),
                 ((-l.half, -l.half), face.corners[3]), ((0, 0), face.center)];
    let r2 = 2 * l.radius;
    for y in 1..rows.saturating_sub(2) {
        for x in 0..cols {
            // The cell's center from the face's center, in half units (both axes doubled).
            let (dx, dy) = (2 * (x as isize - l.x) + 1, 2 * (2 * y as isize + 1 - l.y));
            for &((ix, iy), color) in &discs {
                let d = (dx - 2 * ix).pow(2) + (dy - 2 * iy).pow(2);
                if d <= r2 * r2 { let c = if d > (r2 - 2).pow(2) { RIM } else { color }; grid.put(x, y, ' ', Style::new(c, c)); }
            }
        }
    }
    if show.mode != OrbitMode::Off {
        let orbit = l.half * 3 / 4;
        let color = if show.mode == OrbitMode::Simple { ORBIT_SIMPLE } else { ORBIT_TICKS };
        // The ring: every cell whose center is within half a unit of it.
        for y in 1..rows.saturating_sub(2) {
            for x in 0..cols {
                let (dx, dy) = (2 * (x as isize - l.x) + 1, 2 * (2 * y as isize + 1 - l.y));
                let d = dx * dx + dy * dy;
                if d >= (2 * orbit - 2).pow(2) && d <= (2 * orbit + 2).pow(2) && grid.get(x, y).style == background { grid.put(x, y, '·', Style::new(color, OFF)); }
            }
        }
        for index in 0..if show.mode == OrbitMode::Ticks { 10 } else { 1 } {
            if let Some((x, y)) = cell(&l, point_at(index * 10_000, orbit as usize), cols, rows) {
                grid.put(x, y, if index == 0 { '■' } else { '•' }, Style::new(if index == 0 { START_TICK } else { SMALL_TICK }, OFF));
            }
        }
        if let Some((x, y)) = phase.and_then(|p| cell(&l, point_at(p, orbit as usize), cols, rows)) { grid.put(x, y, '●', Style::new(CYCLE, OFF)); }
    }
    if show.digits && rows >= 2 {
        let text = time_text(seconds.unwrap_or(0));
        let text = if seconds.is_some() { core::str::from_utf8(&text).unwrap_or("") } else { "--:--:--" };
        grid.text_centered(grid.area(), rows - 2, text, Style::new(DIGITS, OFF));
    }
    if show.hints && rows >= 3 {
        grid.text_centered(grid.area(), 0, TITLE, Style::new(HINT, OFF));
        grid.text_centered(grid.area(), rows - 1, if show.switch { KEYS_SWITCH } else { KEYS }, Style::new(HINT, OFF));
    }
}
