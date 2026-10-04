//! Host tests of the file manager (fm/src): masks, sorting, marking, navigation, run/view/quick view/info/find and
//! the menu, over a disk in memory; every state is drawn on small and large grids.
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
#[path = "../fm/src/panel.rs"]
mod panel;
#[path = "../fm/src/fm.rs"]
mod fm;

use abi::*;
use fm::{Disk, Fm, Outcome};
use keys::{event, Key};
use panel::{fat_time, join, matches, parent, Entry, Mode, Panel, Sort};
use tui::viewer::Source;
use tui::{Cell, Grid, CLASSIC};

// 2026-10-04 12:34:56
const STAMP: u32 = ((2026 - 1980) << 9 | 10 << 5 | 4) << 16 | (12 << 11 | 34 << 5 | 28);

struct MemFile(Vec<u8>);
impl Source for MemFile {
    fn size(&self) -> u64 { self.0.len() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize {
        let start = (offset as usize).min(self.0.len());
        let n = out.len().min(self.0.len() - start);
        out[..n].copy_from_slice(&self.0[start..start + n]);
        n
    }
}

#[derive(Default)]
struct Mem { files: Vec<(String, Vec<u8>, u32, u8)>, dirs: Vec<String>, runs: Vec<String>, lists: usize }

impl Mem {
    fn sample() -> Self {
        let mut disk = Mem::default();
        disk.dirs = vec!["EFI".into(), "EFI/BOOT".into(), "docs".into(), "docs/old".into()];
        for (path, data, time) in [("kernel.elf", vec![0x7F, b'E', b'L', b'F'], STAMP), ("top.elf", vec![1; 3000], STAMP - 1), ("readme.txt", "Hello\nПривет, мир\n".as_bytes().to_vec(), STAMP + 1),
                                   ("EFI/BOOT/BOOTX64.EFI", vec![b'M', b'Z'], 0), ("docs/notes.txt", b"notes".to_vec(), 0), ("docs/old/notes.md", b"old".to_vec(), 0),
                                   ("Zeta.TXT", vec![0; 10], STAMP)] {
            disk.files.push((path.into(), data, time, 0));
        }
        disk.files.push(("hidden.sys".into(), vec![], 0, VFS_ENTRY_HIDDEN | VFS_ENTRY_SYSTEM));
        disk
    }
}

impl Disk for Mem {
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String> {
        self.lists += 1;
        if !path.is_empty() && !self.dirs.iter().any(|d| d.eq_ignore_ascii_case(path)) { return Err("NotFound".into()); }
        let mut out: Vec<Entry> = self.dirs.iter().filter(|d| parent(d).0 == path).map(|d| Entry::directory(&parent(d).1)).collect();
        out.extend(self.files.iter().filter(|f| parent(&f.0).0 == path).map(|f| Entry { name: parent(&f.0).1, size: f.1.len() as u64, dir: false, flags: f.3, modified: f.2 }));
        Ok(out)
    }
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>> {
        self.files.iter().find(|f| f.0 == path).map(|f| Box::new(MemFile(f.1.clone())) as Box<dyn Source>)
    }
    fn run(&mut self, path: &str) -> Result<u64, String> { self.runs.push(path.into()); Ok(42) }
}

fn chr(ch: char) -> Key { Key(event(0, ch as u32, 0)) }
fn code(code: u32) -> Key { Key(event(code, 0, 0)) }
fn f(n: u32) -> Key { code(KEY_F1 + n - 1) }
fn ctrl(ch: char) -> Key { Key(event(0, ch as u32, KEY_MOD_CTRL)) }
fn alt_f(n: u32) -> Key { Key(event(KEY_F1 + n - 1, 0, KEY_MOD_ALT)) }

fn draw(fm: &mut Fm, cols: usize, rows: usize) -> Vec<String> {
    let mut cells = vec![Cell::BLANK; cols * rows];
    let mut grid = Grid::new(&mut cells, cols, rows);
    fm.draw(&mut grid, &CLASSIC);
    (0..rows).map(|y| (0..cols).map(|x| grid.get(x, y).ch).collect()).collect()
}

fn screen_has(screen: &[String], text: &str) -> bool { screen.iter().any(|l| l.contains(text)) }

#[test]
fn masks_and_paths() {
    assert!(matches("*.elf", "TOP.ELF"));
    assert!(matches("a?c", "abc") && !matches("a?c", "abbc"));
    assert!(matches("*a*b", "xaxb") && !matches("*a*b", "xaxbx"));
    assert!(matches("*.*", "README"));
    assert!(matches("*.txt, *.md", "notes.md") && !matches("*.txt,*.md", "notes.rs"));
    assert!(matches("при*", "ПРИВЕТ.txt"), "Cyrillic ignores case too");
    assert_eq!(join("", "a"), "a");
    assert_eq!(join("EFI", "BOOT"), "EFI/BOOT");
    assert_eq!(parent("EFI/BOOT"), ("EFI".into(), "BOOT".into()));
    assert_eq!(parent("EFI"), ("".into(), "EFI".into()));
    assert_eq!(fat_time(STAMP), (2026, 10, 4, 12, 34, 56));
    assert_eq!(panel::date_time(STAMP), ("2026-10-04".into(), "12:34".into()));
    assert_eq!(panel::short_size(123_456), "120K");
}

#[test]
fn sorting_filter_and_cursor() {
    let mut disk = Mem::sample();
    let mut p = Panel::new(Mode::Full);
    p.set("", disk.list("").unwrap(), None);
    let names = |p: &Panel| p.items.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&p), ["docs", "EFI", "hidden.sys", "kernel.elf", "readme.txt", "top.elf", "Zeta.TXT"], "directories first, names ignoring case");
    p.hidden = false; p.arrange(None);
    assert!(!names(&p).contains(&"hidden.sys".to_string()));
    p.sort = Sort::Extension; p.arrange(None);
    assert_eq!(names(&p)[2..], ["kernel.elf", "top.elf", "readme.txt", "Zeta.TXT"]);
    p.sort = Sort::Size; p.arrange(None);
    assert_eq!(names(&p)[2], "top.elf", "largest first");
    p.sort = Sort::Time; p.arrange(None);
    assert_eq!(names(&p)[2], "readme.txt", "newest first");
    p.reverse = true; p.arrange(None);
    assert_eq!(names(&p)[..2], ["EFI", "docs"], "directories stay first, reversed among themselves");
    // The cursor stays on its entry when the order changes.
    p.sort = Sort::Name; p.reverse = false; p.arrange(Some("top.elf"));
    assert_eq!(p.current().unwrap().name, "top.elf");
    p.sort = Sort::Size; p.arrange(None);
    assert_eq!(p.current().unwrap().name, "top.elf");
    // A subdirectory starts with "..".
    p.set("docs", disk.list("docs").unwrap(), None);
    assert_eq!(names(&p), ["..", "old", "notes.txt"]);
    assert_eq!(p.list.selected, 0);
}

#[test]
fn marking() {
    let mut disk = Mem::sample();
    let mut p = Panel::new(Mode::Full);
    p.set("", disk.list("").unwrap(), Some("kernel.elf"));
    p.toggle_mark();
    assert!(p.is_marked("kernel.elf"));
    assert_eq!(p.current().unwrap().name, "readme.txt", "Insert moves down");
    assert_eq!(p.mark_mask("*.txt", true), 2);
    assert_eq!(p.marked_size(), (3, 4 + 27 + 10)); // "Привет, мир" is 20 bytes of UTF-8
    p.invert();
    assert!(p.is_marked("top.elf") && p.is_marked("hidden.sys") && !p.is_marked("kernel.elf"));
    assert!(!p.is_marked("docs"), "directories are not marked by masks");
    assert_eq!(p.mark_mask("*", false), 2);
    assert!(p.marked.is_empty());
    // Marks go when the directory changes.
    p.toggle_mark();
    p.set("docs", disk.list("docs").unwrap(), None);
    assert!(p.marked.is_empty());
}

#[test]
fn navigation_open_run_and_view() {
    let mut disk = Mem::sample();
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    let _ = draw(&mut fm, 100, 30);
    // Enter EFI, then BOOT; ".." comes back with the cursor on the directory left.
    fm.key(code(KEY_DOWN), &mut disk);
    assert_eq!(fm.panels[0].current().unwrap().name, "EFI");
    assert_eq!(fm.key(code(KEY_ENTER), &mut disk), Outcome::Redraw);
    assert_eq!(fm.panels[0].path, "EFI");
    fm.key(code(KEY_END), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].path, "EFI/BOOT");
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "A:/EFI/BOOT") && screen_has(&screen, "BOOTX64.EFI"), "{:#?}", screen);
    fm.key(code(KEY_HOME), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!((fm.panels[0].path.as_str(), fm.panels[0].current().unwrap().name.as_str()), ("EFI", "BOOT"));
    fm.key(code(KEY_BACKSPACE), &mut disk);
    assert_eq!((fm.panels[0].path.as_str(), fm.panels[0].current().unwrap().name.as_str()), ("", "EFI"));
    // A program is started through the disk; a text is viewed in the built-in viewer.
    fm.panels[0].arrange(Some("top.elf"));
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(disk.runs, ["top.elf"]);
    assert!(fm.notice.as_deref().unwrap().contains("PID 42"));
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Started top.elf as PID 42"));
    fm.panels[0].arrange(Some("readme.txt"));
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.viewing());
    assert!(fm.status().ends_with("VIEW=1"), "{}", fm.status());
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Привет, мир"), "{:#?}", screen);
    assert_eq!(fm.key(code(KEY_ESC), &mut disk), Outcome::Redraw, "Esc closes the viewer, not fm");
    assert!(!fm.viewing());
    // F3 views again with the same window (it came back from the viewer).
    fm.key(f(3), &mut disk);
    assert!(fm.viewing());
    fm.key(f(10), &mut disk);
    assert!(!fm.viewing());
    // Tab: the other panel.
    fm.key(code(KEY_TAB), &mut disk);
    assert_eq!(fm.active, 1);
    assert!(fm.status().contains("ACTIVE=R"));
    assert_eq!(fm.key(f(10), &mut disk), Outcome::Quit);
}

#[test]
fn quick_view_info_find_and_menu() {
    let mut disk = Mem::sample();
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    fm.panels[0].arrange(Some("readme.txt"));
    fm.key(ctrl('q'), &mut disk);
    assert_eq!(fm.panels[1].mode, Mode::Quick);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Quick view") && screen_has(&screen, "Привет, мир"), "{:#?}", screen);
    fm.key(code(KEY_UP), &mut disk); // kernel.elf
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "ELF"), "quick view follows the cursor");
    fm.key(ctrl('l'), &mut disk);
    assert_eq!(fm.panels[1].mode, Mode::Info);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Directory /") && screen_has(&screen, "kernel.elf") && screen_has(&screen, "modified 2026-10-04 12:34:56"), "{:#?}", screen);
    assert!(screen_has(&screen, "5 files"), "{:#?}", screen);
    // Find from the root: notes.* in docs and docs/old; Enter goes there.
    fm.key(alt_f(7), &mut disk);
    for _ in 0..3 { fm.key(code(KEY_BACKSPACE), &mut disk); }
    for ch in "notes.*".chars() { fm.key(chr(ch), &mut disk); }
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.status().contains("DIALOG=RESULTS"));
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Found 2 for notes.*") && screen_has(&screen, "/docs/old/notes.md"), "{:#?}", screen);
    fm.key(code(KEY_DOWN), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!((fm.panels[0].path.as_str(), fm.panels[0].current().unwrap().name.as_str()), ("docs/old", "notes.md"));
    // F9 opens the menu on the active panel's side; Full -> Brief for the left panel.
    fm.key(f(9), &mut disk);
    assert!(fm.menu.open);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Quick view"));
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].mode, Mode::Brief);
    // + marks by mask, F5 says it is read-only, Alt+F1 lists the volume.
    fm.key(chr('+'), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].marked, ["notes.md"]);
    fm.key(f(5), &mut disk);
    assert!(fm.notice.as_deref().unwrap().contains("Read-only"));
    fm.key(alt_f(1), &mut disk);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "A: boot disk"));
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].path, "");
    // A missing directory leaves the panel as it is, with the error shown.
    fm.load(0, "nothing", None, &mut disk);
    assert_eq!(fm.panels[0].path, "");
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "nothing: NotFound"), "{:#?}", screen);
}

#[test]
fn brief_columns() {
    let mut disk = Mem::sample();
    for i in 0..40 { disk.files.push((format!("f{:02}.txt", i), vec![], 0, 0)); }
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    fm.key(code(KEY_TAB), &mut disk); // the right panel is brief
    let _ = draw(&mut fm, 100, 17); // panels of 15 rows: 10 rows per column, 3 columns
    fm.key(code(KEY_RIGHT), &mut disk);
    assert_eq!(fm.panels[1].list.selected, 10);
    fm.key(code(KEY_END), &mut disk);
    let screen = draw(&mut fm, 100, 17);
    assert!(screen_has(&screen, "Zeta.TXT"), "{:#?}", screen);
    assert_eq!(fm.panels[1].list.top % 10, 0, "brief mode scrolls by columns");
}

#[test]
fn draws_on_any_screen() {
    let mut disk = Mem::sample();
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    let keys = [code(KEY_DOWN), ctrl('q'), code(KEY_END), ctrl('l'), f(1), code(KEY_ESC), f(9), code(KEY_RIGHT), code(KEY_ESC), alt_f(7), code(KEY_ENTER), code(KEY_ESC),
                code(KEY_HOME), code(KEY_ENTER), chr('+'), code(KEY_ESC), f(3), code(KEY_PGDN), code(KEY_ESC), code(KEY_TAB)];
    for (cols, rows) in [(20, 6), (40, 12), (80, 25), (100, 37), (160, 50), (240, 67)] {
        for &key in &keys {
            let _ = fm.key(key, &mut disk);
            let _ = draw(&mut fm, cols, rows);
        }
    }
}
