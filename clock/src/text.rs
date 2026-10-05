//! The text face of `clock` (issue 089): the time in large digits sized to the grid, the weekday and the date under
//! it, the title and the key. Drawn through `mind::tui`: on the program's screen, or in a text window of `wm` that may
//! be resized at any time. No system calls: tests/clock_host.rs.
use crate::tui::{digits, Grid, Style};
use crate::util::FixedBuf;
use core::fmt::Write;

pub const BACKGROUND: u32 = 0x1E1E2E;
pub const FOREGROUND: u32 = 0xA6E3A1;
const DIM: u32 = 0x6C7086;
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// `HH:MM:SS` of seconds since midnight, `--:--:--` without them.
pub fn time_text(seconds: Option<usize>) -> [u8; 8] {
    let Some(s) = seconds.filter(|&s| s < 86400) else { return *b"--:--:--" };
    let digit = |n: usize| b'0' + n as u8;
    [digit(s / 36000), digit(s / 3600 % 10), b':', digit(s / 600 % 6), digit(s / 60 % 10), b':', digit(s % 60 / 10), digit(s % 10)]
}

/// The weekday (0: Monday) of a day number since 2000-01-01, which was a Saturday.
pub fn weekday(days: u32) -> usize { (days as usize + 5) % 7 }

/// Draws the face: `seconds` since midnight (None: the RTC does not answer), the date as (year, month, day, weekday).
pub fn draw(grid: &mut Grid, seconds: Option<usize>, date: Option<(u32, u32, u32, usize)>) {
    let (w, h) = (grid.cols, grid.rows);
    let (style, dim) = (Style::new(FOREGROUND, BACKGROUND), Style::new(DIM, BACKGROUND));
    grid.clear(style);
    let time = time_text(seconds);
    let time = core::str::from_utf8(&time).unwrap_or("--:--:--");
    let mut line = FixedBuf::<48>::new();
    if let Some((year, month, day, weekday)) = date { let _ = write!(line, "{} {}-{:02}-{:02}", WEEKDAYS[weekday % 7], year, month, day); }
    // The title and the key take a row each when there is room; the date one under the digits.
    let framed = h >= 7;
    if framed {
        grid.text_centered(grid.area(), 0, "CLOCK (IPC RTC)", dim);
        grid.text_centered(grid.area(), h - 1, "Esc: exit", dim);
    }
    let rows = if framed { h - 2 } else { h };
    let top = if framed { 1 } else { 0 };
    let date_rows = usize::from(!line.as_str().is_empty() && rows >= 2);
    match digits::fit(time, w.saturating_sub(2), rows.saturating_sub(date_rows + 1).max(1).min(rows), 12) {
        Some(scale) => {
            let (dw, dh) = digits::size(time, scale);
            let gap = usize::from(rows > dh + date_rows);
            let y = top + (rows - dh - date_rows - gap) / 2;
            digits::draw(grid, (w - dw) / 2, y, time, scale, style);
            if date_rows == 1 { grid.text_centered(grid.area(), y + dh + gap, line.as_str(), style); }
        }
        // Too small for large digits: the time as text, the date under it if there is a row.
        None => {
            let y = top + rows.saturating_sub(1 + date_rows) / 2;
            grid.text_centered(grid.area(), y, time, style);
            if date_rows == 1 { grid.text_centered(grid.area(), y + 1, line.as_str(), dim); }
        }
    }
}
