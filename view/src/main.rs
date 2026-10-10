#![no_std]
#![no_main]
// view: text and hex viewer for a file on the disk (mind::tui::viewer). `view <path>`; F1 lists the keys.
use mind::abi::{BootInfo, KEY_DOWN, KEY_F1, KEY_UP, POINTER_LEFT};
use mind::fs::File;
use mind::input::{KeyOrPointer, Pointer};
use mind::keys::{self, Key};
use mind::mem::Pages;
use mind::tui::viewer::{Action, Source, Viewer};
use mind::tui::widgets::fkey_at;
use mind::tui::{Rect, Terminal, CLASSIC};

struct Disk(File);
impl Source for Disk {
    fn size(&self) -> u64 { self.0.size() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { self.0.read_at(offset as usize, out).unwrap_or(0) }
}

// The key a mouse event stands for, and how many times (issue u013): a button of the key bar is its F-key, with the
// modifiers held; the wheel moves three lines a step.
fn pointer_key(p: &Pointer, pressed: bool, area: Rect, modifiers: u8) -> Option<(Key, u32)> {
    if pressed && p.y + 1 == area.bottom() { return Key::from_event(keys::event(KEY_F1 + fkey_at(area.w, p.x) - 1, 0, modifiers)).map(|key| (key, 1)); }
    if p.wheel != 0 { return Key::from_event(keys::event(if p.wheel < 0 { KEY_UP } else { KEY_DOWN }, 0, 0)).map(|key| (key, 3 * p.wheel.unsigned_abs())); }
    None
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("view — text and hex viewer.\nUsage: view <file>\n↑↓ PgUp PgDn Space Home End scroll, F2 wrap, F4 hex or text, F5 go to, F7 search, Shift+F7 next, F1 keys, Esc F3 F10 quit.");
    let path = mind::process::args_str().trim();
    // A status that is not 0 keeps what view said in a window in wm (211-APP-0039): from the menu, with no file, it
    // is not silent (000-APP-0056).
    if path.is_empty() { mind::println!("[VIEW] USAGE: VIEW <FILE>"); mind::process::exit_with(2); }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => { mind::println!("[VIEW] CANNOT OPEN {}: {:?}", path, error); mind::process::exit_with(1); }
    };
    let Some(mut term) = Terminal::open(info, "view") else { return };
    let Some(mut window) = Pages::new(64 * 1024) else { mind::println!("[VIEW] OUT OF MEMORY"); mind::process::exit_with(1) };
    let size = file.size();
    let mut viewer = Viewer::new(Disk(file), window.as_mut_slice(), path);
    mind::println!("[VIEW] OPEN {} {} BYTES", path, size);
    mind::input::pointer(true);
    term.show_pointer(true);
    let mut held = 0u8;
    loop {
        viewer.modifiers = mind::input::modifiers();
        let (cursor, area) = { let mut grid = term.grid(); let area = grid.area(); (viewer.draw(&mut grid, area, &CLASSIC), area) };
        term.set_cursor(cursor);
        term.present();
        mind::println!("[VIEW] TOP {:#X}", viewer.top());
        // Shift going down or up changes the key bar: drawn again.
        let Some(input) = mind::input::wait_key_pointer_or_modifiers(viewer.modifiers) else { continue };
        let (key, times) = match input {
            KeyOrPointer::Key(key) => (key, 1),
            KeyOrPointer::Pointer(p) => {
                let pressed = p.buttons & !held & POINTER_LEFT != 0;
                held = p.buttons;
                match pointer_key(&p, pressed, area, viewer.modifiers) { Some(key) => key, None => continue }
            }
        };
        for _ in 0..times {
            if viewer.key(key) == Action::Quit { mind::println!("[VIEW] DONE"); return; }
        }
    }
}
