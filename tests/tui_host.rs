//! Host tests of the text UI (libmind/src/tui): grid drawing, frames, bars, braille graphs and widgets.
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
use tui::widgets::{fkey_bar, message, Edit, History, InputLine, ListState, MenuAction, MenuBar};
use tui::{Cell, Grid, Line, Rect, Style, CLASSIC};

fn text(grid: &Grid, y: usize) -> String { (0..grid.cols).map(|x| grid.get(x, y).ch).collect::<String>() }
fn k(code: u32) -> Key { Key(event(code, 0, 0)) }
fn c(ch: char) -> Key { Key(event(0, ch as u32, 0)) }
const S: Style = Style::new(1, 2);

#[test]
fn text_is_clipped_and_padded() {
    let mut cells = vec![Cell::BLANK; 10 * 3];
    let mut grid = Grid::new(&mut cells, 10, 3);
    assert_eq!(grid.text(6, 0, "Привет", S), 4);
    assert_eq!(text(&grid, 0), "      Прив");
    grid.text_padded(0, 1, "ab", 5, S);
    assert_eq!(grid.get(4, 1), Cell { ch: ' ', style: S });
    grid.text_right(10, 2, "42", S);
    assert_eq!(text(&grid, 2), "        42");
    grid.text(0, 2, "a\tb", S);
    assert_eq!(grid.get(1, 2).ch, '\u{FFFD}', "control characters are not drawn as such");
}

#[test]
fn frames_and_titles() {
    let mut cells = vec![Cell::BLANK; 12 * 4];
    let mut grid = Grid::new(&mut cells, 12, 4);
    grid.frame_titled(Rect::new(0, 0, 12, 4), Line::Double, "Файлы", S, S);
    assert_eq!(text(&grid, 0), "╔═ Файлы ══╗");
    assert_eq!(text(&grid, 1), "║          ║");
    assert_eq!(text(&grid, 3), "╚══════════╝");
    grid.frame(Rect::new(2, 1, 3, 3), Line::Single, S);
    assert_eq!(&text(&grid, 1)[..], "║ ┌─┐      ║");
}

#[test]
fn bars_use_eighths() {
    let mut cells = vec![Cell::BLANK; 4];
    let mut grid = Grid::new(&mut cells, 4, 1);
    grid.bar(0, 0, 4, 1, 2, S, S);
    assert_eq!(text(&grid, 0), "██  ");
    grid.bar(0, 0, 4, 9, 32, S, S); // 9/32 of 4 cells = 9 eighths
    assert_eq!(text(&grid, 0), "█▏  ");
}

#[test]
fn braille_graph_right_aligns_samples() {
    let mut cells = vec![Cell::BLANK; 3];
    let mut grid = Grid::new(&mut cells, 3, 1);
    grid.graph(Rect::new(0, 0, 3, 1), &[4, 0, 2, 4], 4, S);
    // Six slots, four samples: the first two are empty; 4/4 fills a column, 2/4 its lower half.
    assert_eq!(text(&grid, 0), "\u{2800}\u{2847}\u{28E0}".replace("\u{28E0}", &char::from_u32(0x2800 + 0x40 + 0x04 + 0x80 + 0x20 + 0x10 + 0x08).unwrap().to_string()));
}

#[test]
fn list_state_scrolls_with_selection() {
    let mut state = ListState::default();
    for _ in 0..7 { state.key(k(KEY_DOWN), 20, 5); }
    assert_eq!((state.selected, state.top), (7, 3));
    state.key(k(KEY_PGDN), 20, 5);
    assert_eq!((state.selected, state.top), (11, 7));
    state.key(k(KEY_END), 20, 5);
    assert_eq!((state.selected, state.top), (19, 15));
    state.key(k(KEY_HOME), 20, 5);
    assert_eq!((state.selected, state.top), (0, 0));
    state.select(30, 3, 5);
    assert_eq!((state.selected, state.top), (2, 0), "selection is kept inside a shrunken list");
}

#[test]
fn input_line_edits_utf8_by_character_and_word() {
    let mut line = InputLine::new();
    for ch in "run файл".chars() { assert_eq!(line.key(c(ch)), Edit::Changed); }
    assert_eq!(line.key(k(KEY_LEFT)), Edit::Moved);
    assert_eq!(line.key(k(KEY_BACKSPACE)), Edit::Changed);
    assert_eq!(line.as_str(), "run фал");
    line.key(Key(event(KEY_LEFT, 0, KEY_MOD_CTRL)));
    assert_eq!(line.cursor_chars(), 4);
    line.key(k(KEY_DELETE));
    assert_eq!(line.as_str(), "run ал");
    line.key(k(KEY_END));
    line.key(Key(event(KEY_BACKSPACE, 8, KEY_MOD_ALT)));
    assert_eq!(line.as_str(), "run ");
    assert_eq!(line.key(k(KEY_ENTER)), Edit::Submit);
    assert_eq!(line.key(Key(event(0, 'q' as u32, KEY_MOD_CTRL))), Edit::Ignored, "shortcuts are not text");
    let mut cells = vec![Cell::BLANK; 4];
    let mut grid = Grid::new(&mut cells, 4, 1);
    line.set("abcdefgh");
    assert_eq!(line.draw(&mut grid, 0, 0, 4, S), 3, "scrolled so the cursor is on the last cell");
    assert_eq!(text(&grid, 0), "fgh ");
}

#[test]
fn history_walks_back_and_restores_the_draft() {
    let mut history = History::<3>::new();
    for line in ["one", "two", "two", "", "three", "four"] { history.push(line); }
    assert_eq!(history.len(), 3);
    let mut line = InputLine::new();
    line.set("draft");
    history.key(k(KEY_UP), &mut line); assert_eq!(line.as_str(), "four");
    history.key(k(KEY_UP), &mut line); assert_eq!(line.as_str(), "three");
    history.key(k(KEY_UP), &mut line); assert_eq!(line.as_str(), "two");
    history.key(k(KEY_UP), &mut line); assert_eq!(line.as_str(), "two", "oldest kept");
    history.key(k(KEY_DOWN), &mut line); assert_eq!(line.as_str(), "three");
    history.key(k(KEY_DOWN), &mut line); history.key(k(KEY_DOWN), &mut line);
    assert_eq!(line.as_str(), "draft");
}

#[test]
fn menu_bar_and_key_bar() {
    let items: [&[&str]; 2] = [&["Открыть", "Выход"], &["О программе"]];
    let mut menu = MenuBar::new(&["Файл", "Справка"], &items);
    menu.open = true;
    assert_eq!(menu.key(k(KEY_DOWN)), MenuAction::None);
    assert_eq!(menu.key(k(KEY_ENTER)), MenuAction::Chosen(0, 1));
    menu.open = true;
    menu.key(k(KEY_RIGHT));
    assert_eq!(menu.key(k(KEY_ENTER)), MenuAction::Chosen(1, 0));
    let mut cells = vec![Cell::BLANK; 40 * 6];
    let mut grid = Grid::new(&mut cells, 40, 6);
    menu.open = true; menu.menu = 0; menu.item = 0;
    menu.draw(&mut grid, 0, &CLASSIC);
    assert!(text(&grid, 0).starts_with("   Файл   Справка"));
    assert!(text(&grid, 1).starts_with("  ┌─────────┐"), "{:?}", text(&grid, 1));
    assert_eq!(grid.get(4, 2).style, CLASSIC.menu_selected);
    {
        let mut cells = vec![Cell::BLANK; 80];
        let mut bar = Grid::new(&mut cells, 80, 1);
        fkey_bar(&mut bar, 0, &["Help", "", "View", "", "", "", "", "", "", "Quit"], &CLASSIC);
        assert!(text(&bar, 0).starts_with("1Help   2       3View   "), "{:?}", text(&bar, 0));
        assert!(text(&bar, 0).trim_end().ends_with("10Quit"));
        assert_eq!(bar.get(0, 0).style, CLASSIC.fkey_number);
        assert_eq!(bar.get(1, 0).style, CLASSIC.fkey_label);
    }
    message(&mut grid, "Ошибка", &["Нет файла"], &["OK"], 0, &CLASSIC);
    assert!((0..6).any(|y| text(&grid, y).contains("[ OK ]")));
}

#[test]
fn number_formatting() {
    let mut buffer = [0u8; 32];
    assert_eq!(tui::grouped(1234567, &mut buffer), "1 234 567");
    let mut out = util::FixedBuf::<16>::new();
    tui::human_size(1536, &mut out); assert_eq!(out.as_bytes(), b"1.5K");
    tui::human_size(5 << 30, &mut out); assert_eq!(out.as_bytes(), b"5.0G");
    tui::human_size(999, &mut out); assert_eq!(out.as_bytes(), b"999B");
}
