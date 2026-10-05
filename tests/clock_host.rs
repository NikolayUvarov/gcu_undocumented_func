//! Host tests of the text faces of `clock` and `dzen-clock` (issue 089): what they draw for fixed times, on grids of a
//! screen, of windows and of small windows.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../dzen-clock/src/face.rs"]
mod face;
#[path = "../dzen-clock/src/cycle.rs"]
mod cycle;
#[path = "../dzen-clock/src/text.rs"]
mod dzen;
#[path = "../clock/src/text.rs"]
mod clock;

use cycle::OrbitMode;
use face::{Face, COLORS, OFF, WHITE};
use tui::{Cell, Grid};

fn rows(grid: &Grid) -> Vec<String> { (0..grid.rows).map(|y| (0..grid.cols).map(|x| grid.get(x, y).ch).collect()).collect() }

// 2026-09-19 19:35:05, a Saturday (days since 2000-01-01: 9758).
const SECONDS: usize = 19 * 3600 + 35 * 60 + 5;
const DATE: (u32, u32, u32, usize) = (2026, 9, 19, 5);

#[test]
fn clock_digits_follow_the_grid() {
    assert_eq!(clock::weekday(0), 5, "2000-01-01 was a Saturday");
    assert_eq!(clock::weekday(9758), 5);
    assert_eq!(&clock::time_text(Some(SECONDS)), b"19:35:05");
    assert_eq!(&clock::time_text(None), b"--:--:--");
    for (cols, rows_, scale) in [(80, 25, 2), (160, 50, 5), (78, 22, 2), (38, 9, 1)] {
        let mut cells = vec![Cell::BLANK; cols * rows_];
        let mut grid = Grid::new(&mut cells, cols, rows_);
        clock::draw(&mut grid, Some(SECONDS), Some(DATE));
        let screen = rows(&grid);
        let (w, h) = tui::digits::size("19:35:05", scale);
        let digit_rows: Vec<&String> = screen.iter().filter(|r| r.contains('█') || r.contains('▀') || r.contains('▄')).collect();
        assert_eq!(digit_rows.len(), h, "scale {} on {}x{}: {:#?}", scale, cols, rows_, screen);
        let left = digit_rows.iter().map(|r| r.chars().position(|c| c != ' ').unwrap()).min().unwrap();
        assert_eq!(left, (cols - w) / 2, "centered");
        assert!(screen.iter().any(|r| r.trim() == "Saturday 2026-09-19"), "{:#?}", screen);
        if rows_ >= 7 { assert_eq!(screen[0].trim(), "CLOCK (IPC RTC)"); assert_eq!(screen[rows_ - 1].trim(), "Esc: exit"); }
        assert_eq!(grid.get(0, 0).style.bg, clock::BACKGROUND);
    }
    // Too small for large digits: the time as text.
    let mut cells = vec![Cell::BLANK; 20 * 3];
    let mut grid = Grid::new(&mut cells, 20, 3);
    clock::draw(&mut grid, Some(SECONDS), Some(DATE));
    assert!(rows(&grid).iter().any(|r| r.trim() == "19:35:05"), "{:#?}", rows(&grid));
    clock::draw(&mut grid, None, None);
    assert!(rows(&grid).iter().any(|r| r.trim() == "--:--:--"));
}

// The color of the cells around (x, y) units from the face's center.
fn color_at(grid: &Grid, l: dzen::Layout, x: isize, y: isize) -> u32 {
    let (cx, cy) = ((l.x + x) as usize, ((l.y + y) / 2) as usize);
    grid.get(cx, cy).style.bg
}

#[test]
fn dzen_indicators_orbit_and_keys() {
    // 19:35:05: the top right corner yellow (hour 19 in the fourth quarter of the day), the center cyan (35 minutes),
    // clockwise after it: bottom right dark, bottom left and top left white.
    let face = Face::at(SECONDS).unwrap();
    let show = dzen::Show { digits: true, hints: true, mode: OrbitMode::Off, switch: false };
    let mut cells = vec![Cell::BLANK; 80 * 25];
    let mut grid = Grid::new(&mut cells, 80, 25);
    dzen::draw(&mut grid, face, Some(SECONDS), None, show);
    let l = dzen::layout(80, 25);
    assert_eq!((color_at(&grid, l, l.half, -l.half), color_at(&grid, l, 0, 0)), (COLORS[1], COLORS[3]), "yellow corner, cyan center");
    assert_eq!(color_at(&grid, l, l.half, l.half), OFF);
    assert_eq!((color_at(&grid, l, -l.half, l.half), color_at(&grid, l, -l.half, -l.half)), (WHITE, WHITE));
    let screen = rows(&grid);
    assert_eq!(screen[0].trim(), dzen::TITLE);
    assert_eq!(screen[24].trim(), dzen::KEYS);
    dzen::draw(&mut grid, face, Some(SECONDS), None, dzen::Show { switch: true, ..show });
    assert_eq!(rows(&grid)[24].trim(), dzen::KEYS_SWITCH, "on its own screen T switches to the pixel face");
    dzen::draw(&mut grid, face, Some(SECONDS), None, show);
    assert_eq!(screen[23].trim(), "19:35:05");
    assert!(!screen.iter().any(|r| r.contains('·') || r.contains('●')), "no orbit while it is off");
    // The discs are round: as wide in cells as they are tall in half cells.
    let yellow: Vec<(usize, usize)> = (0..25).flat_map(|y| (0..80).map(move |x| (x, y))).filter(|&(x, y)| grid.get(x, y).style.bg == COLORS[1]).collect();
    let width = yellow.iter().map(|p| p.0).max().unwrap() - yellow.iter().map(|p| p.0).min().unwrap() + 1;
    let height = yellow.iter().map(|p| p.1).max().unwrap() - yellow.iter().map(|p| p.1).min().unwrap() + 1;
    assert!((width as isize - 2 * height as isize).abs() <= 2, "{}x{}", width, height);
    // C: the orbit with its start tick and the dot; P: ten ticks; H and D hide the text.
    dzen::draw(&mut grid, face, Some(SECONDS), Some(5_000), dzen::Show { digits: false, hints: false, mode: OrbitMode::Simple, switch: false });
    let screen = rows(&grid);
    assert!(screen.iter().map(|r| r.matches('·').count()).sum::<usize>() > 10, "{:#?}", screen);
    assert_eq!(screen.iter().map(|r| r.matches('●').count()).sum::<usize>(), 1);
    assert_eq!(screen.iter().map(|r| r.matches('■').count()).sum::<usize>(), 1);
    assert_eq!(screen.iter().map(|r| r.matches('•').count()).sum::<usize>(), 0);
    assert!(screen[0].trim().is_empty() && screen[23].trim().is_empty() && screen[24].trim().is_empty());
    dzen::draw(&mut grid, face, Some(SECONDS), Some(5_000), dzen::Show { digits: true, hints: true, mode: OrbitMode::Ticks, switch: false });
    assert_eq!(rows(&grid).iter().map(|r| r.matches('•').count()).sum::<usize>(), 9, "nine 10-second ticks");
    // The dot moves along the orbit: a quarter turn later it is elsewhere.
    let dot = |grid: &Grid| (0..25).flat_map(|y| (0..80).map(move |x| (x, y))).find(|&(x, y)| grid.get(x, y).ch == '●');
    let first = dot(&grid);
    dzen::draw(&mut grid, face, Some(SECONDS), Some(30_000), dzen::Show { digits: true, hints: true, mode: OrbitMode::Ticks, switch: false });
    assert!(dot(&grid).is_some() && dot(&grid) != first);
    // Small grids still draw (a window being resized).
    for (cols, rows_) in [(10, 3), (20, 6), (38, 9), (160, 50)] {
        let mut cells = vec![Cell::BLANK; cols * rows_];
        let mut grid = Grid::new(&mut cells, cols, rows_);
        dzen::draw(&mut grid, face, None, Some(1), dzen::Show { digits: true, hints: true, mode: OrbitMode::Ticks, switch: false });
    }
}
