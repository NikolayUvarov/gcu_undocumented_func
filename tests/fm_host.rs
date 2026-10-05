//! Host tests of the file manager (fm/src): masks, paths with volumes, sorting, marking, navigation, run/view/quick
//! view/info/find and the menu, copy/move/mkdir/delete jobs with conflicts, failures and stops, and the built-in
//! editor, over a disk in memory (a boot disk whose `data/` is writable and a small `ram:`); every state is drawn on
//! small and large grids.
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
#[path = "../libmind/src/mask.rs"]
mod mask;
#[path = "../libmind/src/pattern.rs"]
mod pattern;
#[path = "../edit/src/buffer.rs"]
mod buffer;
#[path = "../edit/src/editor.rs"]
mod editor;
#[path = "../fm/src/panel.rs"]
mod panel;
#[path = "../fm/src/fm.rs"]
mod fm;

use abi::*;
use fm::{Disk, Failure, Fm, Outcome, Place, Sink, Started, VolumeInfo};
use keys::{event, Key};
use panel::{VFS_ENTRY_HIDDEN, VFS_ENTRY_SYSTEM, display, fat_time, inside, is_root, join, matches, parent, resolve, same_volume, Entry, Mode, Panel, Sort};
use std::cell::{Cell as Counter, RefCell};
use std::rc::Rc;
use tui::viewer::Source;
use tui::{Cell, Grid, CLASSIC};

// 2026-10-04 12:34:56
const STAMP: u32 = ((2026 - 1980) << 9 | 10 << 5 | 4) << 16 | (12 << 11 | 34 << 5 | 28);

type Data = Rc<RefCell<Vec<u8>>>;
fn data(bytes: &[u8]) -> Data { Rc::new(RefCell::new(bytes.to_vec())) }

// A file read in memory; a broken one cannot be read past its first `good` bytes.
struct MemFile { data: Vec<u8>, good: usize }
impl Source for MemFile {
    fn size(&self) -> u64 { self.data.len() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize {
        let start = (offset as usize).min(self.good);
        let n = out.len().min(self.good - start);
        out[..n].copy_from_slice(&self.data[start..start + n]);
        n
    }
}

// A file written in memory; on ram: it takes space.
struct MemSink { data: Data, space: Option<Rc<Counter<u64>>> }
impl Sink for MemSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        if let Some(space) = &self.space {
            if space.get() < bytes.len() as u64 { return Err(Failure::NoSpace); }
            space.set(space.get() - bytes.len() as u64);
        }
        self.data.borrow_mut().extend_from_slice(bytes);
        Ok(())
    }
}

struct Mem { files: Vec<(String, Data, u32, u8)>, dirs: Vec<String>, runs: Vec<String>, args: Vec<String>, lists: usize, flushes: usize, space: Rc<Counter<u64>>, broken: Vec<String> }

const RAM: u64 = 64 * 1024;

impl Mem {
    fn sample() -> Self {
        let mut disk = Mem { files: Vec::new(), dirs: Vec::new(), runs: Vec::new(), args: Vec::new(), lists: 0, flushes: 0, space: Rc::new(Counter::new(RAM)), broken: Vec::new() };
        disk.dirs = vec!["EFI".into(), "EFI/BOOT".into(), "docs".into(), "docs/old".into()];
        for (path, bytes, time) in [("kernel.elf", vec![0x7F, b'E', b'L', b'F'], STAMP), ("top.elf", vec![1; 3000], STAMP - 1), ("readme.txt", "Hello\nПривет, мир\n".as_bytes().to_vec(), STAMP + 1),
                                    ("EFI/BOOT/BOOTX64.EFI", vec![b'M', b'Z'], 0), ("docs/notes.txt", b"notes".to_vec(), 0), ("docs/old/notes.md", b"old".to_vec(), 0),
                                    ("Zeta.TXT", vec![0; 10], STAMP)] {
            disk.files.push((path.into(), data(&bytes), time, 0));
        }
        disk.files.push(("hidden.sys".into(), data(&[]), 0, VFS_ENTRY_HIDDEN | VFS_ENTRY_SYSTEM));
        disk
    }
    fn file(&self, path: &str) -> Option<Vec<u8>> { self.files.iter().find(|f| f.0.eq_ignore_ascii_case(path)).map(|f| f.1.borrow().clone()) }
    fn is_dir(&self, path: &str) -> bool { is_root(path) || self.dirs.iter().any(|d| d.eq_ignore_ascii_case(path)) }
    // As vfs_server allows the shell: ram: and data/ of the boot disk.
    fn allowed(path: &str) -> bool { inside(path, "ram:") || inside(path, "data") }
    fn add_file(&mut self, path: &str, bytes: &[u8]) { self.files.push((path.into(), data(bytes), STAMP, 0)); }
}

impl Disk for Mem {
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String> {
        self.lists += 1;
        if !self.is_dir(path) { return Err("NotFound".into()); }
        let mut out: Vec<Entry> = self.dirs.iter().filter(|d| parent(d).0.eq_ignore_ascii_case(path)).map(|d| Entry::directory(&parent(d).1)).collect();
        out.extend(self.files.iter().filter(|f| parent(&f.0).0.eq_ignore_ascii_case(path)).map(|f| Entry { name: parent(&f.0).1, size: f.1.borrow().len() as u64, dir: false, flags: f.3, modified: f.2 }));
        Ok(out)
    }
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>> {
        let broken = self.broken.iter().any(|b| b.eq_ignore_ascii_case(path));
        self.file(path).map(|bytes| { let good = if broken { bytes.len() / 2 } else { bytes.len() }; Box::new(MemFile { data: bytes, good }) as Box<dyn Source> })
    }
    fn run(&mut self, path: &str, args: &str) -> Result<Started, String> {
        if path.eq_ignore_ascii_case("nothing") { return Err("NotFound".into()); }
        self.runs.push(path.into()); self.args.push(args.into());
        // As in the system: grep is a console program, clock opens a window (fm in wm), the others a screen.
        let place = match path { "grep" => Place::Console, "clock" => Place::Window, _ => Place::Screen };
        Ok(Started { pid: 42, place })
    }
    fn create(&mut self, path: &str, replace: bool) -> Result<Box<dyn Sink>, Failure> {
        if !Self::allowed(path) { return Err(Failure::Denied); }
        if !self.is_dir(&parent(path).0) { return Err(Failure::NotFound); }
        if self.is_dir(path) { return Err(Failure::Other("a directory".into())); }
        let space = path.starts_with("ram:").then(|| self.space.clone());
        if let Some(file) = self.files.iter().find(|f| f.0.eq_ignore_ascii_case(path)) {
            if !replace { return Err(Failure::Exists); }
            if let Some(space) = &space { space.set(space.get() + file.1.borrow().len() as u64); }
            file.1.borrow_mut().clear();
            return Ok(Box::new(MemSink { data: file.1.clone(), space }));
        }
        let bytes = data(&[]);
        self.files.push((path.into(), bytes.clone(), STAMP, 0));
        Ok(Box::new(MemSink { data: bytes, space }))
    }
    fn mkdir(&mut self, path: &str) -> Result<(), Failure> {
        if !Self::allowed(path) { return Err(Failure::Denied); }
        if self.file(path).is_some() { return Err(Failure::Exists); }
        let (volume, rest) = panel::volume(path);
        let mut at = String::from(volume);
        for part in rest.split('/') { at = join(&at, part); if !self.is_dir(&at) { self.dirs.push(at.clone()); } }
        Ok(())
    }
    fn remove(&mut self, path: &str) -> Result<(), Failure> {
        if !Self::allowed(path) { return Err(Failure::Denied); }
        if let Some(i) = self.files.iter().position(|f| f.0.eq_ignore_ascii_case(path)) {
            let (name, bytes, _, _) = self.files.remove(i);
            if name.starts_with("ram:") { self.space.set(self.space.get() + bytes.borrow().len() as u64); }
            return Ok(());
        }
        let Some(i) = self.dirs.iter().position(|d| d.eq_ignore_ascii_case(path)) else { return Err(Failure::NotFound) };
        if !self.list(path).unwrap().is_empty() { return Err(Failure::NotEmpty); }
        self.dirs.remove(i);
        Ok(())
    }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), Failure> {
        if !Self::allowed(from) || !Self::allowed(to) { return Err(Failure::Denied); }
        if !same_volume(from, to) { return Err(Failure::Other("another volume".into())); }
        if self.is_dir(to) || self.file(to).is_some() { return Err(Failure::Exists); }
        if !self.is_dir(&parent(to).0) { return Err(Failure::NotFound); }
        let moved = |p: &str| -> Option<String> { inside(p, from).then(|| format!("{}{}", to, &p[from.len()..])) };
        let mut found = false;
        for d in self.dirs.iter_mut() { if let Some(new) = moved(d) { *d = new; found = true; } }
        for f in self.files.iter_mut() { if let Some(new) = moved(&f.0) { f.0 = new; found = true; } }
        if found { Ok(()) } else { Err(Failure::NotFound) }
    }
    fn writable(&mut self, path: &str) -> bool { Self::allowed(path) && self.file(path).is_some() }
    fn volume(&mut self, path: &str) -> Option<VolumeInfo> {
        Some(if path.starts_with("ram:") { VolumeInfo { label: "MIND RAM".into(), fat_bits: 16, bytes: RAM, free: self.space.get() } }
             else { VolumeInfo { label: "MINDTEST".into(), fat_bits: 16, bytes: 60 << 20, free: 50 << 20 } })
    }
    fn flush(&mut self, _path: &str) { self.flushes += 1; }
}

fn chr(ch: char) -> Key { Key(event(0, ch as u32, 0)) }
fn code(code: u16) -> Key { Key(event(code, 0, 0)) }
fn f(n: u16) -> Key { code(KEY_F1 + n - 1) }
fn ctrl(ch: char) -> Key { Key(event(0, ch as u32, MOD_CTRL)) }
fn alt_f(n: u16) -> Key { Key(event(KEY_F1 + n - 1, 0, MOD_ALT)) }

fn draw(fm: &mut Fm, cols: usize, rows: usize) -> Vec<String> {
    let mut cells = vec![Cell::BLANK; cols * rows];
    let mut grid = Grid::new(&mut cells, cols, rows);
    fm.draw(&mut grid, &CLASSIC);
    (0..rows).map(|y| (0..cols).map(|x| grid.get(x, y).ch).collect()).collect()
}

fn draw_cursor(fm: &mut Fm, cols: usize, rows: usize) -> (Vec<String>, Option<(usize, usize)>) {
    let mut cells = vec![Cell::BLANK; cols * rows];
    let mut grid = Grid::new(&mut cells, cols, rows);
    let cursor = fm.draw(&mut grid, &CLASSIC);
    ((0..rows).map(|y| (0..cols).map(|x| grid.get(x, y).ch).collect()).collect(), cursor)
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
    // The RAM disk: `ram:` is its root.
    assert_eq!(join("ram:", "a"), "ram:a");
    assert_eq!(join("ram:a", "b"), "ram:a/b");
    assert_eq!(parent("ram:a/b"), ("ram:a".into(), "b".into()));
    assert_eq!(parent("ram:a"), ("ram:".into(), "a".into()));
    assert!(is_root("ram:") && is_root("") && !is_root("ram:a"));
    assert_eq!((display(""), display("docs"), display("ram:"), display("ram:a/b")), ("A:/".into(), "A:/docs".into(), "ram:/".into(), "ram:/a/b".into()));
    assert!(same_volume("ram:a", "RAM:b") && !same_volume("ram:a", "a"));
    assert!(inside("docs/old", "docs") && inside("docs", "DOCS") && !inside("docsx", "docs") && inside("ram:x", "ram:") && !inside("x", "ram:"));
    // What the user types in the copy and move dialogs.
    assert_eq!(resolve("docs", "old"), "docs/old");
    assert_eq!(resolve("docs", "A:/data/x"), "data/x");
    assert_eq!(resolve("docs", "a:"), "");
    assert_eq!(resolve("docs", "ram:/x/"), "ram:x");
    assert_eq!(resolve("RAM:", "RAM:x"), "ram:x");
    assert_eq!(resolve("ram:a", "/b"), "ram:b");
    assert_eq!(resolve("", "/b"), "b");
    assert_eq!(resolve("docs/old", ".."), "docs");
    assert_eq!(resolve("docs", "../EFI/./BOOT"), "EFI/BOOT");
    assert_eq!(resolve("ram:x", "../.."), "ram:");
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
    assert!(fm.status().contains("VIEW=1 "), "{}", fm.status());
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
    assert!(screen_has(&screen, "Directory A:/") && screen_has(&screen, "A: boot disk MINDTEST FAT16") && screen_has(&screen, "kernel.elf") && screen_has(&screen, "modified 2026-10-04 12:34:56"), "{:#?}", screen);
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
    // + marks by mask; F5 asks where to copy the marked file (Esc: not); Alt+F1 lists the volumes.
    fm.key(chr('+'), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].marked, ["notes.md"]);
    fm.key(f(5), &mut disk);
    assert!(fm.status().contains("DIALOG=TARGET"), "{}", fm.status());
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Copy notes.md to"), "{:#?}", screen);
    fm.key(code(KEY_ESC), &mut disk);
    fm.key(alt_f(1), &mut disk);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "A: boot disk MINDTEST") && screen_has(&screen, "ram: RAM disk MIND RAM FAT16: 64 KiB, 64 KiB free"), "{:#?}", screen);
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
    for i in 0..40 { disk.add_file(&format!("f{:02}.txt", i), &[]); }
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
                code(KEY_HOME), code(KEY_ENTER), chr('+'), code(KEY_ESC), f(3), code(KEY_PAGE_DOWN), code(KEY_ESC), code(KEY_TAB)];
    for (cols, rows) in [(20, 6), (40, 12), (80, 25), (100, 37), (160, 50), (240, 67)] {
        for &key in &keys {
            let _ = fm.key(key, &mut disk);
            let _ = draw(&mut fm, cols, rows);
        }
    }
}

#[test]
fn key_bars_follow_the_modifiers() {
    let mut disk = Mem::sample();
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    let bar = |fm: &mut Fm, modifiers: u8| { fm.modifiers = modifiers; draw(fm, 100, 30)[29].clone() };
    assert!(bar(&mut fm, 0).starts_with("1Help") && bar(&mut fm, 0).contains("4Edit"), "{}", bar(&mut fm, 0));
    let shifted = bar(&mut fm, MOD_SHIFT);
    assert!(shifted.contains("4New") && !shifted.contains("Help") && !shifted.contains("Quit"), "{}", shifted);
    let ctrl = bar(&mut fm, MOD_CTRL);
    assert!(ctrl.contains("3Name") && ctrl.contains("4Ext") && ctrl.contains("5Time") && ctrl.contains("6Size"), "{}", ctrl);
    let alt = bar(&mut fm, MOD_ALT);
    assert!(alt.contains("1Left") && alt.contains("2Right") && alt.contains("7Find"), "{}", alt);
    // The viewer and the editor inside fm follow them too.
    fm.load(0, "", Some("readme.txt"), &mut disk);
    fm.key(f(3), &mut disk);
    assert!(bar(&mut fm, MOD_SHIFT).contains("7Next"));
    fm.key(f(10), &mut disk);
    fm.key(f(4), &mut disk);
    // A read-only file: the notice covers the bar until a key, but holding Shift shows what the keys do.
    assert!(bar(&mut fm, 0).starts_with("READ-ONLY: on the boot disk only data/ may be changed"), "{}", bar(&mut fm, 0));
    assert!(bar(&mut fm, MOD_SHIFT).contains("2Save as"));
}

#[test]
fn command_line_and_hidden_panels() {
    let mut disk = Mem::sample();
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    // Typing goes to the command line under the panels; the cursor is there.
    typed(&mut fm, &mut disk, "cd docs");
    assert!(fm.status().ends_with(" CMD=cd docs"), "{}", fm.status());
    let (screen, cursor) = draw_cursor(&mut fm, 100, 30);
    assert!(screen[28].starts_with("A:/> cd docs"), "{}", screen[28]);
    assert_eq!(cursor, Some((12, 28)));
    // Left, Backspace, End edit the line, not the panel; Enter runs it.
    fm.key(code(KEY_LEFT), &mut disk);
    fm.key(code(KEY_BACKSPACE), &mut disk);
    typed(&mut fm, &mut disk, "c");
    fm.key(code(KEY_END), &mut disk);
    assert!(fm.status().ends_with(" CMD=cd docs"), "{}", fm.status());
    assert_eq!(fm.key(code(KEY_ENTER), &mut disk), Outcome::Redraw);
    assert_eq!(fm.panels[0].path, "docs");
    assert!(fm.command.is_empty());
    // cd .. comes back with the cursor on the directory left; cd alone goes to the volume root; a missing one is said.
    typed(&mut fm, &mut disk, "cd old");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].path, "docs/old");
    typed(&mut fm, &mut disk, "cd ..");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!((fm.panels[0].path.as_str(), fm.panels[0].current().unwrap().name.as_str()), ("docs", "old"));
    typed(&mut fm, &mut disk, "cd");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].path, "");
    typed(&mut fm, &mut disk, "cd nowhere");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].path, "");
    assert!(fm.notice.as_deref().unwrap().starts_with("cd: A:/nowhere: NotFound"), "{:?}", fm.notice);
    // Esc clears a line; with an empty line it still quits.
    typed(&mut fm, &mut disk, "abc");
    assert_eq!(fm.key(code(KEY_ESC), &mut disk), Outcome::Redraw);
    assert!(fm.command.is_empty());
    // + - * mark on an empty line, and are typed after something.
    fm.key(chr('*'), &mut disk);
    assert!(fm.panels[0].marked.len() > 1);
    fm.key(chr('*'), &mut disk);
    assert!(fm.panels[0].marked.is_empty());
    fm.key(chr(' '), &mut disk);
    assert!(fm.command.is_empty());
    // A program with arguments: a name of this directory becomes its path; Alt+Enter adds the name under the cursor.
    typed(&mut fm, &mut disk, "cd docs");
    fm.key(code(KEY_ENTER), &mut disk);
    fm.panels[0].arrange(Some("notes.txt"));
    typed(&mut fm, &mut disk, "grep -i x");
    fm.key(Key(event(KEY_ENTER, 0, MOD_ALT)), &mut disk);
    assert_eq!(fm.command.as_str(), "grep -i x notes.txt ");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!((disk.runs.last().unwrap().as_str(), disk.args.last().unwrap().as_str()), ("grep", "-i x docs/notes.txt"));
    assert_eq!(fm.notice.as_deref(), Some("Started grep as PID 42: a console program, LOGS 42 in the shell shows what it printed"));
    typed(&mut fm, &mut disk, "cd /");
    fm.key(code(KEY_ENTER), &mut disk);
    typed(&mut fm, &mut disk, "top");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(disk.runs.last().unwrap(), "top.elf", "a program of this directory runs from it");
    assert!(fm.notice.as_deref().unwrap().ends_with("in the background: Ctrl+Z, then FG 42 in the shell shows it"), "{:?}", fm.notice);
    // Where the program shows itself: a window of its own when fm is in one (wm), its log for a console program.
    typed(&mut fm, &mut disk, "clock");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.notice.as_deref(), Some("Started clock (PID 42) in a window of its own"));
    typed(&mut fm, &mut disk, "nothing");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.notice.as_deref().unwrap().starts_with("Cannot start nothing: NotFound"), "{:?}", fm.notice);
    // edit and view open the built-in editor and viewer; a directory is refused.
    typed(&mut fm, &mut disk, "view readme.txt");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.viewing());
    fm.key(f(10), &mut disk);
    typed(&mut fm, &mut disk, "edit ram:new.txt");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.status().starts_with("EDITOR "), "{}", fm.status());
    assert!(!fm.editor.as_ref().unwrap().read_only);
    fm.key(f(10), &mut disk);
    typed(&mut fm, &mut disk, "edit readme.txt");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.editor.as_ref().unwrap().read_only, "an existing file keeps its protection");
    fm.key(f(10), &mut disk);
    typed(&mut fm, &mut disk, "edit docs");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.editor.is_none() && fm.notice.as_deref() == Some("edit: A:/docs is a directory"), "{:?}", fm.notice);

    // Ctrl+O hides both panels: the place shows what the command line did, the cursor is on the line.
    assert_eq!(fm.key(ctrl('o'), &mut disk), Outcome::Redraw);
    assert!(fm.status().ends_with(" HIDDEN=LR"), "{}", fm.status());
    let (screen, cursor) = draw_cursor(&mut fm, 100, 30);
    assert!(screen_has(&screen, "A:/docs> grep -i x notes.txt") && screen_has(&screen, "Started grep as PID 42"), "{:#?}", screen);
    assert!(screen_has(&screen, "edit: A:/docs is a directory") && !screen_has(&screen, "║"), "no panel: {:#?}", screen);
    assert_eq!(cursor, Some((5, 28)));
    // Panel keys do nothing while both are hidden; commands still run.
    let before = fm.panels[0].list.selected;
    fm.key(code(KEY_DOWN), &mut disk);
    assert_eq!(fm.panels[0].list.selected, before);
    typed(&mut fm, &mut disk, "cd docs");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.panels[0].path, "docs");
    fm.key(ctrl('o'), &mut disk);
    assert!(!fm.status().contains("HIDDEN"), "{}", fm.status());
    // Ctrl+F1 / Ctrl+F2: one side; a hidden active panel gives the cursor to the other one.
    let ctrl_f = |n: u16| Key(event(KEY_F1 + n - 1, 0, MOD_CTRL));
    fm.key(ctrl_f(1), &mut disk);
    assert!(fm.status().ends_with(" HIDDEN=L") && fm.status().contains("ACTIVE=R"), "{}", fm.status());
    let screen = draw(&mut fm, 100, 30);
    let half = |line: &String, right: bool| -> String { line.chars().skip(if right { 50 } else { 0 }).take(50).collect() };
    assert!(half(&screen[0], false).starts_with("A:/> cd docs") && half(&screen[0], true).contains("═ A:/ ═") && !half(&screen[5], false).contains('║'), "{:#?}", screen);
    fm.key(code(KEY_TAB), &mut disk);
    assert!(fm.status().contains("ACTIVE=R"), "Tab does not go to a hidden panel: {}", fm.status());
    fm.key(ctrl_f(1), &mut disk);
    fm.key(ctrl_f(2), &mut disk);
    assert!(fm.status().ends_with(" HIDDEN=R") && fm.status().contains("ACTIVE=L"), "{}", fm.status());
    fm.key(ctrl_f(2), &mut disk);
    // Ctrl+P: the other panel.
    fm.key(ctrl('p'), &mut disk);
    assert!(fm.status().ends_with(" HIDDEN=R"), "{}", fm.status());
    fm.key(ctrl('p'), &mut disk);
    assert!(!fm.status().contains("HIDDEN"), "{}", fm.status());
    // Ctrl+O with one panel hidden hides both; again shows both.
    fm.key(ctrl('p'), &mut disk);
    fm.key(ctrl('o'), &mut disk);
    assert!(fm.status().ends_with(" HIDDEN=LR"), "{}", fm.status());
    fm.key(ctrl('o'), &mut disk);
    assert!(!fm.status().contains("HIDDEN"), "{}", fm.status());
    // Every state draws on small screens.
    for (cols, rows) in [(20, 8), (40, 12), (100, 30)] { fm.key(ctrl('o'), &mut disk); let _ = draw(&mut fm, cols, rows); fm.key(ctrl_f(2), &mut disk); let _ = draw(&mut fm, cols, rows); }
}

fn shift_f(n: u16) -> Key { Key(event(KEY_F1 + n - 1, 0, MOD_SHIFT)) }
fn alt_f2() -> Key { alt_f(2) }
fn typed(fm: &mut Fm, disk: &mut Mem, text: &str) { for ch in text.chars() { fm.key(chr(ch), disk); } }
fn clear_line(fm: &mut Fm, disk: &mut Mem) { for _ in 0..40 { fm.key(code(KEY_BACKSPACE), disk); } }

// Runs the job until it ends or waits for an answer; returns the number of slices.
fn run(fm: &mut Fm, disk: &mut Mem) -> usize {
    let mut slices = 0;
    while fm.busy() { fm.work(disk); slices += 1; assert!(slices < 10_000, "the job does not end"); }
    slices
}

fn ram_panel(fm: &mut Fm, disk: &mut Mem) {
    fm.key(alt_f2(), disk);
    fm.key(code(KEY_DOWN), disk);
    fm.key(code(KEY_ENTER), disk);
    assert_eq!(fm.panels[1].path, "ram:");
}

#[test]
fn copy_move_mkdir_delete() {
    let mut disk = Mem::sample();
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    ram_panel(&mut fm, &mut disk);
    // F5 on docs: the dialog offers the other panel; the tree is copied to ram:.
    fm.panels[0].arrange(Some("docs"));
    fm.key(f(5), &mut disk);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Copy docs to") && screen_has(&screen, "ram:/"), "{:#?}", screen);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.status().contains("JOB=COPY:0/4"), "{}", fm.status());
    run(&mut fm, &mut disk);
    assert_eq!(disk.file("ram:docs/notes.txt").unwrap(), b"notes");
    assert_eq!(disk.file("ram:docs/old/notes.md").unwrap(), b"old");
    assert_eq!(fm.notice.as_deref(), Some("Copied 4 of 4, 8 bytes"));
    assert_eq!(fm.panels[1].current().unwrap().name, "docs", "the cursor is on the copy");
    assert!(fm.status().ends_with("JOB=NONE"));
    assert!(disk.flushes >= 2);
    // Again: the target exists; Skip leaves the files as they are.
    disk.files.iter().find(|f| f.0 == "ram:docs/notes.txt").unwrap().1.borrow_mut().extend_from_slice(b"!");
    disk.space.set(disk.space.get() - 1);
    fm.key(f(5), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.status().contains("DIALOG=OVERWRITE"));
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "1 of the targets exist already."), "{:#?}", screen);
    fm.key(code(KEY_RIGHT), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    assert_eq!(disk.file("ram:docs/notes.txt").unwrap(), b"notes!");
    assert!(fm.notice.as_deref().unwrap().ends_with("2 skipped"), "{:?}", fm.notice);
    // Overwrite replaces them.
    fm.key(f(5), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    assert_eq!(disk.file("ram:docs/notes.txt").unwrap(), b"notes");
    // Not onto itself, not into itself.
    fm.key(code(KEY_TAB), &mut disk);
    fm.panels[1].arrange(Some("docs"));
    fm.key(f(5), &mut disk);
    clear_line(&mut fm, &mut disk);
    typed(&mut fm, &mut disk, "ram:/");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.notice.as_deref(), Some("Cannot copy ram:/docs onto itself"));
    fm.key(f(5), &mut disk);
    clear_line(&mut fm, &mut disk);
    typed(&mut fm, &mut disk, "ram:/docs/old");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.notice.as_deref(), Some("Cannot put ram:/docs inside itself"));
    // F6 in place: a new name on the same volume is one rename.
    fm.key(f(6), &mut disk);
    clear_line(&mut fm, &mut disk);
    typed(&mut fm, &mut disk, "papers");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.status().contains("JOB=MOVE:0/1"), "{}", fm.status());
    run(&mut fm, &mut disk);
    assert!(disk.file("ram:papers/old/notes.md").is_some() && disk.file("ram:docs/notes.txt").is_none());
    assert_eq!(fm.panels[1].current().unwrap().name, "papers");
    // F7: a directory on the boot disk only in data/.
    fm.key(code(KEY_TAB), &mut disk);
    fm.key(f(7), &mut disk);
    typed(&mut fm, &mut disk, "system");
    fm.key(code(KEY_ENTER), &mut disk);
    assert_eq!(fm.notice.as_deref(), Some("Cannot make A:/system: denied (only ram: and data/ are writable)"));
    fm.key(f(7), &mut disk);
    typed(&mut fm, &mut disk, "data/inbox");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(disk.is_dir("data/inbox"));
    assert_eq!(fm.panels[0].current().unwrap().name, "data");
    // F6 across volumes: copied, then the source removed.
    fm.key(code(KEY_TAB), &mut disk);
    fm.key(f(6), &mut disk);
    clear_line(&mut fm, &mut disk);
    typed(&mut fm, &mut disk, "A:/data");
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    assert_eq!(disk.file("data/papers/old/notes.md").unwrap(), b"old");
    assert!(!disk.is_dir("ram:papers") && disk.list("ram:").unwrap().is_empty(), "{:?}", disk.dirs);
    assert_eq!(disk.space.get(), RAM, "the RAM disk is empty again");
    assert!(fm.notice.as_deref().unwrap().starts_with("Moved 8 of 8"), "{:?}", fm.notice);
    // F8: the tree goes after a confirmation; Cancel keeps it.
    fm.key(code(KEY_TAB), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk); // into data
    fm.panels[0].arrange(Some("papers"));
    fm.key(f(8), &mut disk);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Delete 1 entry") && screen_has(&screen, "(4 files and directories in all)?"), "{:#?}", screen);
    fm.key(code(KEY_RIGHT), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(disk.is_dir("data/papers"));
    fm.key(f(8), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    assert!(!disk.is_dir("data/papers") && disk.file("data/papers/notes.txt").is_none());
    assert_eq!(fm.notice.as_deref(), Some("Deleted 4 of 4"));
    // Marked files from the boot disk to ram:; a move from the boot disk cannot remove the sources.
    fm.key(code(KEY_BACKSPACE), &mut disk);
    fm.panels[0].mark_mask("*.txt", true);
    fm.key(f(6), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    assert!(fm.status().contains("JOB=MOVE:2/4:FAILED"), "{}", fm.status());
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Delete A:/readme.txt") && screen_has(&screen, "failed: denied") && screen_has(&screen, "Retry"), "{:#?}", screen);
    fm.key(code(KEY_RIGHT), &mut disk); // Skip
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    fm.key(code(KEY_RIGHT), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.job.is_none());
    assert!(disk.file("ram:readme.txt").is_some() && disk.file("ram:Zeta.TXT").is_some() && disk.file("readme.txt").is_some());
    assert!(fm.notice.as_deref().unwrap().ends_with("2 skipped, 2 failed"), "{:?}", fm.notice);
    assert!(fm.panels[0].marked.is_empty());
}

#[test]
fn failures_space_and_stopping() {
    let mut disk = Mem::sample();
    disk.add_file("data/big.bin", &vec![7u8; 200 * 1024]);
    disk.dirs.push("data".into());
    disk.add_file("data/half.bin", &vec![9u8; 100 * 1024]);
    disk.broken.push("data/half.bin".into());
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    ram_panel(&mut fm, &mut disk);
    fm.load(0, "data", Some("half.bin"), &mut disk);
    // A file that cannot be read: Retry fails again, Skip goes on and removes the partial copy.
    fm.key(f(5), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    run(&mut fm, &mut disk);
    assert!(fm.status().contains(":FAILED"), "{}", fm.status());
    fm.key(code(KEY_ENTER), &mut disk); // Retry
    run(&mut fm, &mut disk);
    assert!(fm.status().contains(":FAILED"));
    fm.key(code(KEY_RIGHT), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.job.is_none());
    assert!(disk.file("ram:half.bin").is_none(), "the partial copy is removed");
    assert_eq!(disk.space.get(), RAM);
    // A file larger than the RAM disk: the disk is full; Abort stops and removes the partial copy.
    fm.panels[0].arrange(Some("big.bin"));
    fm.key(f(5), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(run(&mut fm, &mut disk) >= 1);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "failed: the disk is full"), "{:#?}", screen);
    fm.key(code(KEY_LEFT), &mut disk); // Abort
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.notice.as_deref().unwrap().starts_with("Stopped: 0 of 1"), "{:?}", fm.notice);
    assert!(disk.file("ram:big.bin").is_none());
    // Esc during a copy asks; Continue goes on, Stop ends it.
    disk.space.set(1 << 30);
    fm.key(f(5), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    fm.work(&mut disk);
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Copying") && screen_has(&screen, "32%") && screen_has(&screen, "Esc: stop"), "{:#?}", screen);
    fm.key(code(KEY_ESC), &mut disk);
    assert!(!fm.busy() && fm.status().contains(":ASKING"));
    let screen = draw(&mut fm, 100, 30);
    assert!(screen_has(&screen, "Stop the operation?"), "{:#?}", screen);
    fm.key(code(KEY_ENTER), &mut disk); // Continue (selected)
    assert!(fm.busy());
    fm.work(&mut disk);
    fm.key(code(KEY_ESC), &mut disk);
    fm.key(code(KEY_LEFT), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.job.is_none());
    assert!(fm.notice.as_deref().unwrap().starts_with("Stopped:"));
    // A copy that ends normally.
    fm.key(f(5), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk);
    fm.key(code(KEY_ENTER), &mut disk); // overwrite the partial copy left by Stop
    assert_eq!(run(&mut fm, &mut disk), 4, "64 KiB a slice");
    assert_eq!(disk.file("ram:big.bin").unwrap(), vec![7u8; 200 * 1024]);
}

#[test]
fn built_in_editor() {
    let mut disk = Mem::sample();
    disk.dirs.push("ram:notes".into());
    disk.add_file("ram:notes/todo.txt", "купить хлеб\n".as_bytes());
    let mut window = vec![0u8; 4096];
    let mut fm = Fm::new(&mut window, &mut disk);
    fm.load(0, "ram:notes", Some("todo.txt"), &mut disk);
    fm.key(f(4), &mut disk);
    assert!(fm.status().starts_with("EDITOR LINE=1 COL=1 BYTES=22 LINES=2 MODIFIED=0"), "{}", fm.status());
    let screen = draw(&mut fm, 100, 30);
    assert!(screen[1].starts_with("купить хлеб") && screen[0].contains("ram:notes/todo.txt"), "{:#?}", &screen[..2]);
    fm.key(Key(event(KEY_END, 0, MOD_CTRL)), &mut disk);
    typed(&mut fm, &mut disk, "и молоко");
    fm.key(f(2), &mut disk);
    assert_eq!(disk.file("ram:notes/todo.txt").unwrap(), "купить хлеб\nи молоко".as_bytes());
    assert!(disk.file("ram:notes/todo.txt.tmp").is_none());
    fm.key(f(10), &mut disk);
    assert!(fm.editor.is_none());
    assert_eq!(fm.panels[0].current().unwrap().name, "todo.txt");
    assert_eq!(fm.panels[0].current().unwrap().size, 37);
    // Shift+F4: a new file; F10 asks, Save saves and closes.
    fm.key(shift_f(4), &mut disk);
    typed(&mut fm, &mut disk, "new.txt");
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.status().contains("BYTES=0"));
    typed(&mut fm, &mut disk, "Hello");
    fm.key(f(10), &mut disk);
    assert!(fm.status().contains("DIALOG=UNSAVED"));
    fm.key(code(KEY_ENTER), &mut disk);
    assert!(fm.editor.is_none());
    assert_eq!(disk.file("ram:notes/new.txt").unwrap(), b"Hello");
    assert_eq!(fm.panels[0].current().unwrap().name, "new.txt");
    // A boot file opens read-only.
    fm.load(0, "", Some("readme.txt"), &mut disk);
    fm.key(f(4), &mut disk);
    typed(&mut fm, &mut disk, "x");
    assert!(fm.status().contains("MODIFIED=0"));
    let screen = draw(&mut fm, 100, 30);
    assert!(screen[0].contains("READ-ONLY  Ln 1"), "{}", screen[0]);
    assert!(fm.editor.as_ref().unwrap().notice.as_deref().unwrap().contains("READ-ONLY"));
    fm.key(f(10), &mut disk);
    assert!(fm.editor.is_none());
    // F4 on a directory says what it does.
    fm.panels[0].arrange(Some("docs"));
    fm.key(f(4), &mut disk);
    assert!(fm.notice.as_deref().unwrap().contains("Shift+F4"));
}

#[test]
fn draws_jobs_and_dialogs_on_any_screen() {
    for (cols, rows) in [(20, 6), (40, 12), (80, 25), (160, 50)] {
        let mut disk = Mem::sample();
        disk.dirs.push("data".into());
        disk.add_file("data/big.bin", &vec![1u8; 150 * 1024]);
        let mut window = vec![0u8; 4096];
        let mut fm = Fm::new(&mut window, &mut disk);
        ram_panel(&mut fm, &mut disk);
        fm.load(0, "data", Some("big.bin"), &mut disk);
        for keys in [vec![f(5)], vec![f(6)], vec![f(7)], vec![shift_f(4)], vec![f(8)], vec![alt_f(1)]] {
            for key in keys { fm.key(key, &mut disk); }
            let _ = draw(&mut fm, cols, rows);
            fm.key(code(KEY_ESC), &mut disk);
        }
        fm.key(f(5), &mut disk);
        fm.key(code(KEY_ENTER), &mut disk);
        fm.work(&mut disk);
        let _ = draw(&mut fm, cols, rows);
        fm.key(code(KEY_ESC), &mut disk);
        let _ = draw(&mut fm, cols, rows);
        fm.key(code(KEY_RIGHT), &mut disk);
        fm.key(code(KEY_ENTER), &mut disk);
        run(&mut fm, &mut disk);
        let _ = draw(&mut fm, cols, rows);
        fm.key(f(5), &mut disk);
        fm.key(code(KEY_ENTER), &mut disk);
        let _ = draw(&mut fm, cols, rows);
    }
}
