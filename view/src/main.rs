#![no_std]
#![no_main]
// view: text and hex viewer for a file on the disk (mind::tui::viewer). `view <path>`; F1 lists the keys.
use mind::abi::BootInfo;
use mind::fs::File;
use mind::gfx::Screen;
use mind::mem::Pages;
use mind::tui::viewer::{Action, Source, Viewer};
use mind::tui::{Terminal, CLASSIC};

struct Disk(File);
impl Source for Disk {
    fn size(&self) -> u64 { self.0.size() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { self.0.read_at(offset as usize, out).unwrap_or(0) }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let path = mind::process::args_str().trim();
    if path.is_empty() { mind::println!("[VIEW] USAGE: VIEW <FILE>"); return; }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => { mind::println!("[VIEW] CANNOT OPEN {}: {:?}", path, error); return; }
    };
    let Some(mut term) = Screen::new(info).and_then(Terminal::new) else { return };
    let Some(mut window) = Pages::new(64 * 1024) else { mind::println!("[VIEW] OUT OF MEMORY"); return };
    let size = file.size();
    let mut viewer = Viewer::new(Disk(file), window.as_mut_slice(), path);
    mind::println!("[VIEW] OPEN {} {} BYTES", path, size);
    loop {
        viewer.modifiers = mind::input::modifiers();
        let cursor = { let mut grid = term.grid(); let area = grid.area(); viewer.draw(&mut grid, area, &CLASSIC) };
        term.set_cursor(cursor);
        term.present();
        mind::println!("[VIEW] TOP {:#X}", viewer.top());
        // Shift going down or up changes the key bar: drawn again.
        let Some(key) = mind::input::wait_key_or_modifiers(viewer.modifiers) else { continue };
        if viewer.key(key) == Action::Quit { mind::println!("[VIEW] DONE"); return; }
    }
}
