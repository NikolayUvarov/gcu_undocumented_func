//! Host tests of the editor (edit/src): the piece table against a plain byte vector, undo/redo groups, search and
//! replace, the editor's keys and dialogs, and drawing on small and large grids.
#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../edit/src/buffer.rs"]
mod buffer;
#[path = "../edit/src/editor.rs"]
mod editor;

use abi::*;
use buffer::Buffer;
use editor::{Editor, Outcome};
use keys::{event, Key};
use tui::{Cell, Grid, CLASSIC};

fn chr(ch: char) -> Key { Key(event(0, ch as u32, 0)) }
fn code(code: u16) -> Key { Key(event(code, 0, 0)) }
fn shift(code: u16) -> Key { Key(event(code, 0, MOD_SHIFT)) }
fn ctrl_code(code: u16) -> Key { Key(event(code, 0, MOD_CTRL)) }
fn f(n: u16) -> Key { code(KEY_F1 + n - 1) }
fn fmod(n: u16, mods: u8) -> Key { Key(event(KEY_F1 + n - 1, 0, mods)) }
fn ctrl(ch: char) -> Key { Key(event(0, ch as u32, MOD_CTRL)) }
fn enter() -> Key { Key(event(KEY_ENTER, '\n' as u32, 0)) }

fn typed(editor: &mut Editor, text: &str) { for c in text.chars() { if c == '\n' { editor.key(enter()); } else { editor.key(chr(c)); } } }
fn text(editor: &Editor) -> String { String::from_utf8_lossy(&editor.buffer.text()).into_owned() }

fn draw(editor: &mut Editor, cols: usize, rows: usize) -> (Vec<String>, Vec<Cell>, Option<(usize, usize)>) {
    let mut cells = vec![Cell::BLANK; cols * rows];
    let cursor = { let mut grid = Grid::new(&mut cells, cols, rows); editor.draw(&mut grid, &CLASSIC) };
    let lines = cells.chunks(cols).map(|row| row.iter().map(|c| c.ch).collect()).collect();
    (lines, cells, cursor)
}
fn screen_has(screen: &[String], text: &str) -> bool { screen.iter().any(|l| l.contains(text)) }

// Line starts recomputed from the text.
fn lines_of(text: &[u8]) -> Vec<usize> { let mut v = vec![0]; v.extend(text.iter().enumerate().filter(|(_, &b)| b == b'\n').map(|(i, _)| i + 1)); v }
fn check_lines(buffer: &Buffer) {
    let text = buffer.text();
    let starts = lines_of(&text);
    assert_eq!(buffer.line_count(), starts.len());
    for (i, &start) in starts.iter().enumerate() { assert_eq!(buffer.line_start(i), start, "line {}", i); }
    for at in 0..=text.len() { assert_eq!(buffer.line_of(at), starts.partition_point(|&s| s <= at) - 1); }
}

#[test]
fn piece_table_keeps_bytes_and_lines() {
    let original = b"one\r\ntwo\r\n\xFFbad\xC3\r\nlast".to_vec();
    let mut b = Buffer::new(original.clone());
    assert_eq!(b.text(), original);
    assert!(b.crlf());
    assert_eq!(b.line_count(), 4);
    assert_eq!((b.line_start(1), b.line_end(1)), (5, 8)); // "two" without CR LF
    assert_eq!(b.line_end(3), b.len());
    // Invalid bytes are characters of their own.
    assert_eq!(b.char_at(10), Some(('\u{FFFD}', 1)));
    assert_eq!(b.next(10), 11);
    assert_eq!(b.previous(11), 10);
    b.insert(3, "-три".as_bytes(), false);
    assert_eq!(b.text(), "one-три\r\ntwo\r\n\u{FFFD}".bytes().take(0).chain(b"one-\xD1\x82\xD1\x80\xD0\xB8\r\ntwo\r\n\xFFbad\xC3\r\nlast".iter().copied()).collect::<Vec<u8>>());
    check_lines(&b);
    // Cyrillic steps by characters.
    assert_eq!(b.next(4), 6);
    assert_eq!(b.previous(6), 4);
    let removed = b.delete(0, 10, false);
    assert_eq!(removed, b"one-\xD1\x82\xD1\x80\xD0\xB8".to_vec());
    check_lines(&b);
    assert_eq!(&b.text()[..3], b"\r\nt");
    // Undo brings everything back byte for byte.
    b.undo(); b.undo();
    assert_eq!(b.text(), original);
    assert!(!b.modified());
    check_lines(&b);
}

#[test]
fn random_edits_match_a_byte_vector() {
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut random = move |n: usize| { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % n.max(1) as u64) as usize };
    let alphabet: [&[u8]; 6] = [b"a", b"\n", b"\r\n", "ж".as_bytes(), b"xyz", b"\xFF"];
    let mut b = Buffer::new(b"start\nof the\ntext\n".to_vec());
    let mut model = b.text();
    let mut history: Vec<Vec<u8>> = vec![model.clone()];
    let mut future: Vec<Vec<u8>> = Vec::new();
    for step in 0..3000 {
        match random(10) {
            0..=4 => {
                let at = random(model.len() + 1);
                let mut data = Vec::new();
                for _ in 0..1 + random(3) { data.extend_from_slice(alphabet[random(alphabet.len())]); }
                b.insert(at, &data, false);
                model.splice(at..at, data.iter().copied());
                history.push(model.clone()); future.clear();
            }
            5..=7 if !model.is_empty() => {
                let start = random(model.len());
                let end = (start + 1 + random(6)).min(model.len());
                assert_eq!(b.delete(start, end, false), model[start..end].to_vec());
                model.drain(start..end);
                history.push(model.clone()); future.clear();
            }
            8 if history.len() > 1 => { b.undo().unwrap(); future.push(history.pop().unwrap()); model = history.last().unwrap().clone(); }
            9 if !future.is_empty() => { b.redo().unwrap(); model = future.pop().unwrap(); history.push(model.clone()); }
            _ => {}
        }
        assert_eq!(b.len(), model.len(), "step {}", step);
        if step % 50 == 0 { assert_eq!(b.text(), model, "step {}", step); check_lines(&b); }
    }
    assert_eq!(b.text(), model);
    check_lines(&b);
    let mut out = vec![0u8; 7];
    let n = b.read(model.len() / 2, &mut out);
    assert_eq!(&out[..n], &model[model.len() / 2..(model.len() / 2 + 7).min(model.len())]);
}

#[test]
fn undo_groups_and_the_save_point() {
    let mut e = Editor::new(Vec::new(), "ram:a.txt", false);
    typed(&mut e, "hello world");
    assert!(e.buffer.modified());
    // Typing is one group: one undo removes it all.
    e.key(ctrl('u'));
    assert_eq!(text(&e), "");
    assert!(!e.buffer.modified());
    e.key(ctrl('y'));
    assert_eq!(text(&e), "hello world");
    assert_eq!(e.cursor, 11);
    // A newline ends the group; moving the cursor too.
    typed(&mut e, "\nsecond");
    e.key(code(KEY_LEFT));
    typed(&mut e, "!");
    assert_eq!(text(&e), "hello world\nsecon!d");
    e.key(ctrl('u'));
    assert_eq!(text(&e), "hello world\nsecond");
    e.key(Key(event(KEY_BACKSPACE, 8, MOD_ALT))); // Alt+Backspace undoes too
    assert_eq!(text(&e), "hello world\n");
    e.key(ctrl('u'));
    assert_eq!(text(&e), "hello world");
    // The save point: saved, changed, undone back to it.
    e.saved("ram:a.txt", Ok(11));
    assert!(!e.buffer.modified());
    typed(&mut e, "X");
    assert!(e.buffer.modified());
    e.key(ctrl('u'));
    assert!(!e.buffer.modified());
    // Undo past the save point, then a new change: the saved text cannot come back by redo.
    e.key(ctrl('u'));
    assert!(e.buffer.modified());
    typed(&mut e, "Z");
    e.key(ctrl('u'));
    assert!(e.buffer.modified());
    assert!(e.status().contains("MODIFIED=1"));
}

#[test]
fn search_and_replace_ignore_case() {
    let mut b = Buffer::new("Привет, мир! ПРИВЕТ снова. privet\nПривет".as_bytes().to_vec());
    let first = b.find("привет", 0).unwrap();
    assert_eq!(first, (0, "Привет".len()));
    let second = b.find("привет", first.1).unwrap();
    assert_eq!(&b.text()[second.0..second.1], "ПРИВЕТ".as_bytes());
    // Wraps around to the start.
    let last = b.find("привет", second.1).unwrap();
    assert_eq!(b.find("привет", last.1), Some(first));
    assert_eq!(b.find("нет такого", 0), None);
    let all = b.find_all("привет");
    assert_eq!(all.len(), 3);
    b.replace_all(&all, "hi".as_bytes());
    assert_eq!(String::from_utf8(b.text()).unwrap(), "hi, мир! hi снова. privet\nhi");
    check_lines(&b);
    // Replace all is one undo step.
    b.undo();
    assert_eq!(String::from_utf8(b.text()).unwrap(), "Привет, мир! ПРИВЕТ снова. privet\nПривет");
    b.redo();
    assert_eq!(String::from_utf8(b.text()).unwrap(), "hi, мир! hi снова. privet\nhi");
    b.replace(0, 2, b"Hello");
    assert_eq!(&b.text()[..7], b"Hello, ");
    b.undo();
    assert_eq!(&b.text()[..4], b"hi, ");
}

#[test]
fn editing_keys() {
    let mut e = Editor::new(b"    indented line\nsecond\n".to_vec(), "data/t.txt", false);
    // Home goes to the indentation, then to the line start.
    e.key(code(KEY_END));
    assert_eq!(e.cursor, 17);
    e.key(code(KEY_HOME));
    assert_eq!(e.cursor, 4);
    e.key(code(KEY_HOME));
    assert_eq!(e.cursor, 0);
    e.key(code(KEY_HOME));
    assert_eq!(e.cursor, 4);
    // Enter keeps the indentation.
    e.key(code(KEY_END));
    typed(&mut e, "\nnext");
    assert_eq!(text(&e), "    indented line\n    next\nsecond\n");
    // Words.
    e.key(ctrl_code(KEY_HOME));
    e.key(ctrl_code(KEY_RIGHT));
    assert_eq!(e.cursor, 4); // the next word's start
    e.key(ctrl_code(KEY_RIGHT));
    assert_eq!(e.cursor, 13);
    e.key(ctrl_code(KEY_LEFT));
    assert_eq!(e.cursor, 4);
    // Shift selects; Ctrl+C / Ctrl+V; typing replaces the selection.
    for _ in 0..8 { e.key(shift(KEY_RIGHT)); }
    assert_eq!(e.selection(), Some((4, 12)));
    e.key(ctrl('c'));
    e.key(ctrl_code(KEY_END));
    e.key(ctrl('v'));
    assert!(text(&e).ends_with("second\nindented"));
    e.key(ctrl('a'));
    typed(&mut e, "Ж");
    assert_eq!(text(&e), "Ж");
    e.key(ctrl('u'));
    assert!(text(&e).starts_with("    indented"));
    // Ctrl+X cuts.
    e.key(ctrl_code(KEY_HOME));
    for _ in 0..4 { e.key(shift(KEY_RIGHT)); }
    e.key(ctrl('x'));
    assert!(text(&e).starts_with("indented line"));
    // Up/Down keep the column; Cyrillic counts as one column.
    let mut e = Editor::new("абвгд\nx\nабвгд".as_bytes().to_vec(), "", false);
    e.key(code(KEY_RIGHT)); e.key(code(KEY_RIGHT)); e.key(code(KEY_RIGHT));
    assert_eq!(e.cursor, 6);
    e.key(code(KEY_DOWN));
    assert_eq!(e.buffer.line_of(e.cursor), 1);
    e.key(code(KEY_DOWN));
    assert_eq!(e.cursor, "абвгд\nx\nабв".len());
    assert!(e.status().starts_with("LINE=3 COL=4"));
    // Backspace and Delete take a CRLF as one; a new line in a CRLF file is CRLF.
    let mut e = Editor::new(b"a\r\nb\r\n".to_vec(), "", false);
    e.key(code(KEY_DOWN));
    e.key(code(KEY_BACKSPACE));
    assert_eq!(e.buffer.text(), b"ab\r\n");
    e.key(code(KEY_LEFT));
    e.key(code(KEY_END));
    e.key(code(KEY_DELETE));
    assert_eq!(e.buffer.text(), b"ab");
    e.key(enter());
    assert_eq!(e.buffer.text(), b"ab\n"); // no line ending left to follow: LF
    let mut e = Editor::new(b"a\r\nb".to_vec(), "", false);
    e.key(code(KEY_END));
    e.key(enter());
    assert_eq!(e.buffer.text(), b"a\r\n\r\nb");
    // Left and Right step over a CR LF at once.
    e.key(ctrl_code(KEY_HOME));
    e.key(code(KEY_END));
    e.key(code(KEY_RIGHT));
    assert_eq!(e.cursor, 3);
    e.key(code(KEY_LEFT));
    assert_eq!(e.cursor, 1);
    // Overwrite replaces characters but not the line end.
    let mut e = Editor::new("abc\nd".as_bytes().to_vec(), "", false);
    e.key(code(KEY_INSERT));
    typed(&mut e, "XYZW");
    assert_eq!(text(&e), "XYZW\nd");
    // Tab, PgDn, Ctrl+End.
    let long: String = (1..=100).map(|n| format!("line {}\n", n)).collect();
    let mut e = Editor::new(long.into_bytes(), "", false);
    draw(&mut e, 80, 25);
    e.key(code(KEY_PAGE_DOWN));
    assert_eq!(e.line(), 22);
    e.key(ctrl_code(KEY_END));
    assert_eq!(e.line(), 100);
    e.key(code(KEY_TAB));
    assert!(text(&e).ends_with("line 100\n\t"));
}

#[test]
fn key_bar_follows_the_modifiers() {
    let mut e = Editor::new(b"text".to_vec(), "ram:a.txt", false);
    let bar = |e: &mut Editor, modifiers: u8| { e.modifiers = modifiers; draw(e, 100, 10).0[9].clone() };
    assert!(bar(&mut e, 0).starts_with("1Help     2Save     3"), "{}", bar(&mut e, 0));
    let shifted = bar(&mut e, MOD_SHIFT);
    assert!(shifted.contains("2Save as") && shifted.contains("7Next") && !shifted.contains("Help"), "{}", shifted);
    assert!(bar(&mut e, MOD_CTRL).contains("7Replace"));
    assert!(bar(&mut e, MOD_ALT).contains("8Go to"));
    assert!(bar(&mut e, MOD_CTRL | MOD_SHIFT).starts_with("1Help"), "two modifiers: the plain bar");
}

#[test]
fn read_only_refuses_changes() {
    let mut e = Editor::new(b"boot file".to_vec(), "kernel.elf", true);
    // It says so at once, and READ-ONLY stays in the status line.
    let (screen, _, _) = draw(&mut e, 100, 10);
    assert!(screen[9].starts_with("READ-ONLY: this file cannot be changed here"), "{}", screen[9]);
    typed(&mut e, "x");
    e.key(code(KEY_DELETE));
    e.key(ctrl('v'));
    assert_eq!(text(&e), "boot file");
    assert!(e.notice.as_deref().unwrap().contains("READ-ONLY"));
    let (screen, cells, _) = draw(&mut e, 100, 10);
    let at = screen[0][..screen[0].find("READ-ONLY").expect("READ-ONLY in the status line")].chars().count();
    assert_eq!(cells[at].style, CLASSIC.error, "it stands out");
    // Save as elsewhere is allowed.
    e.key(fmod(2, MOD_SHIFT));
    assert!(e.status().contains("DIALOG=SAVEAS"));
    for _ in 0..20 { e.key(code(KEY_BACKSPACE)); }
    typed(&mut e, "ram:copy.elf");
    assert_eq!(e.key(enter()), Outcome::Save(String::from("ram:copy.elf")));
    e.saved("ram:copy.elf", Ok(9));
    assert!(!e.read_only);
    assert_eq!(e.path, "ram:copy.elf");
    typed(&mut e, "!");
    assert_eq!(text(&e), "!boot file");
}

#[test]
fn saving_quitting_and_dialogs() {
    let mut e = Editor::new(Vec::new(), "", false);
    // Quit with nothing changed.
    assert_eq!(e.key(f(10)), Outcome::Quit);
    typed(&mut e, "Привет");
    // F2 on a new text asks for a name.
    assert_eq!(e.key(f(2)), Outcome::Redraw);
    assert!(e.status().contains("DIALOG=SAVEAS"));
    typed(&mut e, "ram:new.txt");
    assert_eq!(e.key(enter()), Outcome::Save(String::from("ram:new.txt")));
    e.saved("ram:new.txt", Err(String::from("the disk is full")));
    assert!(e.buffer.modified());
    assert!(e.notice.as_deref().unwrap().contains("Not saved"));
    // F10 with changes: Save / Don't save / Cancel; Save without a name asks for one first.
    assert_eq!(e.key(f(10)), Outcome::Redraw);
    assert!(e.status().contains("DIALOG=UNSAVED"));
    assert_eq!(e.key(enter()), Outcome::Redraw);
    assert!(e.status().contains("DIALOG=SAVEAS"));
    typed(&mut e, "ram:new.txt");
    assert_eq!(e.key(enter()), Outcome::SaveAndQuit(String::from("ram:new.txt")));
    e.saved("ram:new.txt", Err(String::from("denied"))); // the program stays when the save fails
    assert!(e.status().contains("DIALOG=NONE"));
    e.key(f(10));
    e.key(code(KEY_RIGHT));
    assert_eq!(e.key(enter()), Outcome::Quit);
    e.dialog = None;
    e.key(f(10));
    e.key(code(KEY_RIGHT)); e.key(code(KEY_RIGHT));
    assert_eq!(e.key(enter()), Outcome::Redraw);
    assert!(e.status().contains("DIALOG=NONE"));
    e.key(code(KEY_ESC)); // Esc quits like F10: the dialog again, Esc there cancels
    assert!(e.status().contains("DIALOG=UNSAVED"));
    e.key(code(KEY_ESC));
    assert!(e.status().contains("DIALOG=NONE"));
    // Once saved under a name, F2 saves directly.
    e.key(f(2));
    typed(&mut e, "ram:new.txt");
    assert_eq!(e.key(enter()), Outcome::Save(String::from("ram:new.txt")));
    e.saved("ram:new.txt", Ok(12));
    assert!(!e.buffer.modified());
    typed(&mut e, "!");
    assert_eq!(e.key(f(2)), Outcome::Save(String::from("ram:new.txt")));
    e.saved("ram:new.txt", Ok(13));
    assert_eq!(e.key(f(10)), Outcome::Quit);
}

#[test]
fn find_replace_goto_and_menu() {
    let mut e = Editor::new("alpha beta\nГамма beta\nend".as_bytes().to_vec(), "x.txt", false);
    e.key(f(7));
    typed(&mut e, "BETA");
    e.key(enter());
    assert_eq!(e.selection(), Some((6, 10)));
    e.key(fmod(7, MOD_SHIFT));
    let at = "alpha beta\nГамма ".len();
    assert_eq!(e.selection(), Some((at, at + 4)));
    e.key(fmod(7, MOD_SHIFT));
    assert_eq!(e.selection(), Some((6, 10))); // wrapped
    e.key(f(7));
    for _ in 0..10 { e.key(code(KEY_BACKSPACE)); }
    typed(&mut e, "нет");
    e.key(enter());
    assert!(e.notice.as_deref().unwrap().contains("Not found"));
    // Ctrl+F7: replace all, one undo step.
    e.key(fmod(7, MOD_CTRL));
    for _ in 0..10 { e.key(code(KEY_BACKSPACE)); }
    typed(&mut e, "гамма");
    e.key(enter());
    typed(&mut e, "delta");
    e.key(enter());
    assert_eq!(text(&e), "alpha beta\ndelta beta\nend");
    assert!(e.notice.as_deref().unwrap().contains("Replaced 1"));
    e.key(ctrl('u'));
    assert_eq!(text(&e), "alpha beta\nГамма beta\nend");
    // Alt+F8: go to line.
    e.key(fmod(8, MOD_ALT));
    typed(&mut e, "3");
    e.key(enter());
    assert_eq!(e.line(), 2);
    e.key(fmod(8, MOD_ALT));
    typed(&mut e, "x");
    e.key(enter());
    assert!(e.notice.is_some());
    // F9: Edit → Select all, then Options → overwrite.
    e.key(f(9));
    assert!(e.status().contains("MENU=1"));
    e.key(code(KEY_RIGHT));
    for _ in 0..5 { e.key(code(KEY_DOWN)); }
    e.key(enter());
    assert_eq!(e.selection(), Some((0, e.buffer.len())));
    e.key(f(9));
    e.key(code(KEY_LEFT));
    e.key(code(KEY_DOWN));
    e.key(enter());
    assert!(e.overwrite);
    // F1: the keys.
    e.key(f(1));
    assert!(e.status().contains("DIALOG=HELP"));
    e.key(code(KEY_ESC));
    assert!(e.status().contains("DIALOG=NONE"));
}

#[test]
fn draws_text_status_and_dialogs() {
    let mut bytes = "Привет\tмир\nline ".as_bytes().to_vec();
    bytes.extend_from_slice(b"\xFFtwo\n"); // an invalid byte in the middle of the second line
    let mut e = Editor::new(bytes, "ram:hello.txt", false);
    let (screen, cells, cursor) = draw(&mut e, 80, 25);
    assert!(screen[0].contains("ram:hello.txt"));
    assert!(screen[0].contains("Ln 1 Col 1"));
    assert!(screen[0].contains("LF"));
    assert!(screen[1].starts_with("Привет  мир")); // the tab expands to column 8
    assert!(screen[2].starts_with("line \u{FFFD}"));
    assert_eq!(cells[2 * 80 + 5].style, CLASSIC.error);
    assert!(screen[24].contains("Save") && screen[24].contains("Quit"));
    assert_eq!(cursor, Some((0, 1)));
    // A selection is drawn selected.
    e.key(shift(KEY_RIGHT)); e.key(shift(KEY_RIGHT));
    let (_, cells, _) = draw(&mut e, 80, 25);
    assert_eq!(cells[80].style, CLASSIC.selected);
    assert_eq!(cells[81].style, CLASSIC.selected);
    assert_ne!(cells[82].style, CLASSIC.selected);
    // Modified, overwrite and read-only show in the status line.
    typed(&mut e, "Й");
    e.key(code(KEY_INSERT));
    let (screen, _, cursor) = draw(&mut e, 80, 25);
    assert!(screen[0].contains("*") && screen[0].contains("OVR"));
    assert_eq!(cursor, Some((1, 1)));
    // A long line scrolls horizontally to keep the cursor visible.
    let long = "x".repeat(200);
    let mut e = Editor::new(long.into_bytes(), "", false);
    e.key(code(KEY_END));
    let (screen, _, cursor) = draw(&mut e, 40, 10);
    assert_eq!(cursor, Some((39, 1)));
    assert!(screen[0].contains("(new)"));
    // Many lines scroll vertically.
    let many: String = (1..=50).map(|n| format!("row {}\n", n)).collect();
    let mut e = Editor::new(many.into_bytes(), "", false);
    e.key(ctrl_code(KEY_END));
    let (screen, _, cursor) = draw(&mut e, 40, 10);
    assert_eq!(cursor, Some((0, 8)));
    assert!(screen[7].starts_with("row 50"));
    // Every dialog and the menu draw on small and large grids.
    for (cols, rows) in [(20, 6), (40, 12), (80, 25), (160, 50)] {
        let mut e = Editor::new("text\n".as_bytes().to_vec(), "f.txt", false);
        typed(&mut e, "a");
        for keys in [vec![f(1)], vec![f(7)], vec![fmod(7, MOD_CTRL)], vec![fmod(8, MOD_ALT)],
                     vec![fmod(2, MOD_SHIFT)], vec![f(10)], vec![f(9), code(KEY_RIGHT), code(KEY_DOWN)]] {
            for key in keys { e.key(key); }
            let (screen, _, _) = draw(&mut e, cols, rows);
            assert_eq!(screen.len(), rows);
            e.dialog = None; e.menu.open = false;
        }
    }
    let mut e = Editor::new(Vec::new(), "", false);
    e.key(f(10));
    typed(&mut e, "q");
    e.key(f(10));
    let (screen, _, _) = draw(&mut e, 80, 25);
    assert!(screen_has(&screen, "Don't save"));
}

#[test]
fn quit_from_the_menu_and_the_key_bar() {
    // Issue u013: File > Quit (Enter or a click), F10 with the menu open, and a click on the key bar's "10 Quit" all
    // do what F10 does: an unchanged file ends the editor, a changed one asks first.
    let fresh = || { let mut e = Editor::new(b"one\ntwo\n\tthree\n".to_vec(), "ram:a.txt", false); draw(&mut e, 80, 25); e };
    let mut editor = fresh();
    editor.key(f(9));
    editor.key(code(KEY_DOWN)); editor.key(code(KEY_DOWN));
    assert_eq!(editor.key(enter()), Outcome::Quit, "File > Quit with Enter");
    let mut editor = fresh();
    editor.key(f(9));
    assert_eq!(editor.key(f(10)), Outcome::Quit, "F10 while the menu is open, as the item says");
    let mut editor = fresh();
    editor.key(f(9));
    let (screen, _, _) = draw(&mut editor, 80, 25);
    let row = screen.iter().position(|l| l.contains("Quit  F10")).expect("the File menu is drawn");
    let column = screen[row].find("Quit").map(|byte| screen[row][..byte].chars().count()).unwrap();
    assert_eq!(editor.pointer(column, row, 0, 0), Outcome::Redraw, "the mouse over Quit highlights it");
    assert_eq!(editor.menu.item, 2);
    assert_eq!(editor.pointer(column + 1, row, 0, 0), Outcome::Ignored, "moving over the same item changes nothing");
    assert_eq!(editor.pointer(column, row, POINTER_LEFT as u8, 0), Outcome::Quit, "File > Quit clicked");
    let mut editor = fresh();
    assert_eq!(editor.pointer(75, 24, POINTER_LEFT as u8, 0), Outcome::Quit, "10 Quit clicked on the key bar");
    let mut editor = fresh();
    typed(&mut editor, "x");
    assert_eq!(editor.pointer(75, 24, POINTER_LEFT as u8, 0), Outcome::Redraw, "a changed text asks first");
    assert!(editor.dialog.is_some());
    // A click on a menu title opens it, elsewhere closes it; a click in the text puts the cursor there; the wheel.
    let mut editor = fresh();
    editor.key(f(9));
    editor.pointer(0, 0, 0, 0);
    let (screen, _, _) = draw(&mut editor, 80, 25);
    let edit_title = screen[0].find("Edit").map(|byte| screen[0][..byte].chars().count()).unwrap();
    assert_eq!(editor.pointer(edit_title, 0, POINTER_LEFT as u8, 0), Outcome::Redraw);
    assert!(editor.menu.open && editor.menu.menu == 1, "the Edit menu");
    editor.pointer(edit_title, 0, 0, 0);
    assert_eq!(editor.pointer(60, 20, POINTER_LEFT as u8, 0), Outcome::Redraw);
    assert!(!editor.menu.open, "closed by a click outside it");
    editor.pointer(60, 20, 0, 0);
    editor.pointer(6, 3, POINTER_LEFT as u8, 0);
    assert_eq!(editor.cursor, b"one\ntwo\n\tth".len(), "row 3 is the third line; column 6 is inside \"three\" after the tab");
    editor.pointer(6, 3, 0, 0);
    editor.pointer(0, 1, POINTER_LEFT as u8, 0);
    assert_eq!(editor.cursor, 0);
    editor.pointer(0, 1, 0, 0);
    assert_eq!(editor.pointer(0, 1, 0, 1), Outcome::Redraw);
    assert_eq!(editor.line(), 3, "one wheel step: three lines down (the last, empty line after the final newline)");
}
