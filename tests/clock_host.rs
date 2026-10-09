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

#[test]
fn the_dzen_clock_on_one_line() {
    // Issue u016: the time, then the discs' colors — top left and right, the center, bottom left and right.
    let seconds = 12 * 3600 + 34 * 60 + 56; // hour 12: red at the top left; step 2: white at the bottom left; cyan center
    let (bytes, n) = dzen::line(face::Face::at(seconds).unwrap(), seconds);
    assert_eq!(core::str::from_utf8(&bytes[..n]).unwrap(), "12:34:56  R· C W·");
    let (bytes, n) = dzen::line(face::Face::at(0).unwrap(), 0);
    assert_eq!(core::str::from_utf8(&bytes[..n]).unwrap(), "00:00:00  ·· R WR", "hour 0: red at the bottom right, step 0: white at the bottom left");
}

#[path = "../libmind/src/wallclock.rs"]
mod wallclock;

#[test]
fn the_rtc_is_read_once_a_minute_and_seconds_are_counted_between() {
    // 000-APP-0012: a clock asks 10 times a second; the slow clock (RTC) changes its second 0.35 s after the start.
    let rtc = |t: u64| Some((43_200 + (t + 650_000_000) / 1_000_000_000) as usize % 86_400);
    let mut clock = wallclock::WallClock::new();
    let (mut reads, mut shown) = (0, Vec::new());
    for tick in 0..6_000u64 { // ten minutes at 100 ms
        let t = tick * 100_000_000;
        let s = clock.seconds(t, || { reads += 1; rtc(t) }).unwrap();
        assert!(s.abs_diff(rtc(t).unwrap()) <= 1, "{} s off at {} ms", s.abs_diff(rtc(t).unwrap()), t / 1_000_000);
        if shown.last() != Some(&s) { shown.push(s); }
    }
    assert!(reads <= 10 * 6 + 10, "{} reads in ten minutes", reads); // about six per resynchronization, not 6000
    assert_eq!(shown.len(), 601, "every second shown once, from 12:00:00 to 12:10:00");
    assert!(shown.windows(2).all(|w| w[1] == w[0] + 1));
}

#[test]
fn midnight_wraps_and_marks_the_date_due() {
    let mut clock = wallclock::WallClock::new();
    assert_eq!(clock.seconds(0, || Some(86_398)), Some(86_398));
    assert!(clock.date_due());
    assert_eq!(clock.seconds(100_000_000, || Some(86_399)), Some(86_399)); // the second changed: counted from here
    assert!(!clock.date_due());
    assert_eq!(clock.seconds(1_200_000_000, || panic!("not read between resynchronizations")), Some(0));
    assert!(clock.date_due(), "past midnight the date is read again");
    assert!(!clock.date_due());
}

#[test]
fn a_stuck_or_missing_rtc() {
    // A clock whose second never changes is taken as it is after 1.5 s; one that does not answer gives None.
    let mut clock = wallclock::WallClock::new();
    let mut reads = 0;
    for tick in 0..40u64 { clock.seconds(tick * 100_000_000, || { reads += 1; Some(100) }); }
    assert!(reads <= 17, "{} reads", reads);
    assert_eq!(clock.seconds(5_000_000_000, || Some(100)), Some(103));
    assert_eq!(wallclock::WallClock::new().seconds(0, || None), None);
}
