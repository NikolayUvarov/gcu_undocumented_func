//! Host tests of the viewer core (libmind/src/tui/viewer.rs) over a file in memory.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
use abi::*;
use keys::{event, Key};
use tui::viewer::{Action, Mode, Source, Viewer};
use tui::{Cell, Grid, Rect, CLASSIC};

struct Memory(Vec<u8>, usize); // data, reads made
impl Source for Memory {
    fn size(&self) -> u64 { self.0.len() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize {
        self.1 += 1;
        let start = (offset as usize).min(self.0.len());
        let n = out.len().min(self.0.len() - start);
        out[..n].copy_from_slice(&self.0[start..start + n]);
        n
    }
}
fn k(code: u16) -> Key { Key(event(code, 0, 0)) }
fn row(grid: &Grid, y: usize) -> String { (0..grid.cols).map(|x| grid.get(x, y).ch).collect::<String>().trim_end().to_string() }
fn text_file(lines: usize) -> Vec<u8> { (1..=lines).map(|i| format!("строка {i} line {i}\n")).collect::<String>().into_bytes() }

fn draw(viewer: &mut Viewer<Memory>, cells: &mut Vec<Cell>) -> Vec<String> {
    let mut grid = Grid::new(cells, 60, 12);
    viewer.draw(&mut grid, Rect::new(0, 0, 60, 12), &CLASSIC);
    (0..12).map(|y| row(&grid, y)).collect()
}

#[test]
fn text_pages_scroll_and_end() {
    let mut buf = vec![0u8; 4096];
    let mut viewer = Viewer::new(Memory(text_file(300), 0), &mut buf, "t.txt");
    let mut cells = vec![Cell::BLANK; 60 * 12];
    let screen = draw(&mut viewer, &mut cells);
    assert!(screen[0].contains("t.txt") && screen[0].contains("Стр 1 "), "{screen:?}");
    assert_eq!(screen[1], "строка 1 line 1");
    viewer.key(k(KEY_DOWN));
    assert_eq!(draw(&mut viewer, &mut cells)[1], "строка 2 line 2");
    viewer.key(k(KEY_PAGE_DOWN));
    let screen = draw(&mut viewer, &mut cells);
    assert_eq!(screen[1], "строка 11 line 11");
    assert!(screen[0].contains("Стр 11 "));
    viewer.key(k(KEY_END));
    let screen = draw(&mut viewer, &mut cells);
    assert_eq!(screen[10], "строка 300 line 300", "the last page ends with the last line: {screen:?}");
    viewer.key(k(KEY_DOWN));
    assert_eq!(draw(&mut viewer, &mut cells)[10], "строка 300 line 300", "no scrolling past the end");
    viewer.key(k(KEY_PAGE_UP));
    assert_eq!(draw(&mut viewer, &mut cells)[1], "строка 282 line 282");
    assert!(draw(&mut viewer, &mut cells)[0].contains("Стр 282 "), "line numbers follow scrolling up");
    viewer.key(k(KEY_HOME));
    assert_eq!(draw(&mut viewer, &mut cells)[1], "строка 1 line 1");
}

#[test]
fn wrapping_and_horizontal_scroll() {
    let long = format!("{}\nnext\n", "abcdefghij".repeat(15)); // 150 characters
    let mut buf = vec![0u8; 4096];
    let mut viewer = Viewer::new(Memory(long.into_bytes(), 0), &mut buf, "long");
    let mut cells = vec![Cell::BLANK; 60 * 12];
    let screen = draw(&mut viewer, &mut cells);
    assert_eq!(screen[1].chars().count(), 60);
    assert_eq!(screen[4], "next", "150 characters wrap into three rows");
    viewer.key(k(KEY_DOWN));
    assert!(draw(&mut viewer, &mut cells)[1].starts_with("abcdefghij"), "scrolling moves by visual rows");
    viewer.key(k(KEY_F1 + 1)); // F2: no wrap
    let screen = draw(&mut viewer, &mut cells);
    assert_eq!(screen[2], "next");
    assert!(screen[1].ends_with('»'), "a cut line is marked");
    viewer.key(k(KEY_RIGHT));
    assert!(draw(&mut viewer, &mut cells)[1].starts_with("ijabcdefgh"));
}

#[test]
fn hex_mode_and_goto() {
    let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    let mut buf = vec![0u8; 1024];
    let mut viewer = Viewer::new(Memory(data, 0), &mut buf, "bin");
    let mut cells = vec![Cell::BLANK; 80 * 12];
    viewer.key(k(KEY_F1 + 3)); // F4
    assert_eq!(viewer.mode, Mode::Hex);
    let mut grid = Grid::new(&mut cells, 80, 12);
    viewer.draw(&mut grid, Rect::new(0, 0, 80, 12), &CLASSIC);
    assert!(row(&grid, 1).starts_with("00000000: 00 01 02 03 04 05 06 07  08 09"), "{}", row(&grid, 1));
    assert!(row(&grid, 3).contains("!\"#$%&'()*+,-./"));
    viewer.key(k(KEY_F1 + 4)); // F5: go to
    for ch in "1A0".chars() { viewer.key(Key(event(0, ch as u32, 0))); }
    viewer.key(Key(event(KEY_ENTER, 10, 0)));
    assert_eq!(viewer.top(), 0x1A0);
    viewer.key(k(KEY_F1 + 4));
    for ch in "50%".chars() { viewer.key(Key(event(0, ch as u32, 0))); }
    viewer.key(Key(event(KEY_ENTER, 10, 0)));
    assert_eq!(viewer.top(), 2048);
}

#[test]
fn search_finds_case_insensitively_and_wraps() {
    let mut buf = vec![0u8; 512]; // a small window: the search crosses window boundaries
    let mut viewer = Viewer::new(Memory(text_file(200), 0), &mut buf, "t");
    let mut cells = vec![Cell::BLANK; 60 * 12];
    draw(&mut viewer, &mut cells);
    viewer.key(k(KEY_F1 + 6)); // F7
    for ch in "LINE 150".chars() { viewer.key(Key(event(0, ch as u32, 0))); }
    viewer.key(Key(event(KEY_ENTER, 10, 0)));
    let screen = draw(&mut viewer, &mut cells);
    assert!(screen[1].starts_with("строка 150 line 150"), "{screen:?}");
    viewer.key(Key(event(KEY_F1 + 6, 0, MOD_SHIFT)));
    assert!(draw(&mut viewer, &mut cells)[1].starts_with("строка 150 line 150"), "the next match is in the same line");
    viewer.key(Key(event(KEY_F1 + 6, 0, MOD_SHIFT)));
    assert!(draw(&mut viewer, &mut cells)[1].starts_with("строка 150 line 150"), "only one line matches: the search wraps to it");
    // Cyrillic ignores case too.
    viewer.key(k(KEY_F1 + 6));
    for _ in 0..10 { viewer.key(k(KEY_BACKSPACE)); }
    for ch in "СТРОКА 77 ".chars() { viewer.key(Key(event(0, ch as u32, 0))); }
    viewer.key(Key(event(KEY_ENTER, 10, 0)));
    assert!(draw(&mut viewer, &mut cells)[1].starts_with("строка 77 line 77"));
    viewer.key(k(KEY_F1 + 6));
    for _ in 0..12 { viewer.key(k(KEY_BACKSPACE)); }
    for ch in "нет такого".chars() { viewer.key(Key(event(0, ch as u32, 0))); }
    viewer.key(Key(event(KEY_ENTER, 10, 0)));
    assert!(draw(&mut viewer, &mut cells).iter().any(|r| r.contains("Не найдено")));
    assert_eq!(viewer.key(k(KEY_ESC)), Action::Quit);
}

#[test]
fn invalid_utf8_and_tabs() {
    let mut buf = vec![0u8; 256];
    let mut viewer = Viewer::new(Memory(b"a\tb\xff\xd0c\n".to_vec(), 0), &mut buf, "x");
    let mut cells = vec![Cell::BLANK; 60 * 12];
    assert_eq!(draw(&mut viewer, &mut cells)[1], "a       b\u{FFFD}\u{FFFD}c");
}
