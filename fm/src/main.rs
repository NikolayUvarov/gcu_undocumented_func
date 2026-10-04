#![no_std]
#![no_main]
// fm: file manager over vfs_server (fm::fm). Programs it starts go through the loader's launch session with the
// standard grants and run in the background.
extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use fm::fm::{Disk, Fm, Outcome};
use fm::panel::Entry;
use mind::fs::{self, File};
use mind::gfx::Screen;
use mind::idl::{loader, wire};
use mind::ipc::Endpoint;
use mind::tui::viewer::Source;
use mind::tui::{Terminal, CLASSIC};

struct DiskFile(File);
impl Source for DiskFile {
    fn size(&self) -> u64 { self.0.size() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { self.0.read_at(offset as usize, out).unwrap_or(0) }
}

struct Vfs { shared: Option<wire::Shared> }

impl Disk for Vfs {
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        fs::list(path, |e| entries.push(Entry { name: String::from_utf8_lossy(e.name).into(), size: e.size as u64, dir: e.is_dir, flags: e.flags, modified: e.modified }))
            .map_err(|error| alloc::format!("{:?}", error))?;
        Ok(entries)
    }
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>> { File::open(path).ok().map(|file| Box::new(DiskFile(file)) as Box<dyn Source>) }
    fn run(&mut self, path: &str) -> Result<u64, String> {
        let shared = self.shared.as_mut().ok_or_else(|| String::from("no memory"))?;
        let failed = |e: loader::Error| alloc::format!("{:?}", e);
        let session = loader::begin(Endpoint::LOADER, shared.buffer(), path, "").map_err(|e| alloc::format!("{:?}", e))?.map_err(failed)?;
        loader::commit(Endpoint::LOADER, session).map_err(|e| alloc::format!("{:?}", e))?.map_err(failed)
    }
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    let Some(mut term) = Screen::new(info).and_then(Terminal::new) else { return };
    let mut disk = Vfs { shared: wire::Shared::new(4096).ok() };
    // The viewer's window lives as long as the program; the viewer gives it back when it closes.
    let window: &'static mut [u8] = Box::leak(alloc::vec![0u8; 64 * 1024].into_boxed_slice());
    let mut fm = Fm::new(window, &mut disk);
    // `fm <directory>` opens the left panel there.
    let start = mind::process::args_str().trim().trim_matches('/');
    if !start.is_empty() { fm.load(0, start, None, &mut disk); }
    mind::println!("[FM] READY {}", fm.status());
    loop {
        let cursor = { let mut grid = term.grid(); fm.draw(&mut grid, &CLASSIC) };
        term.set_cursor(cursor);
        term.present();
        let key = loop { if let Some(key) = mind::input::wait_key(1000) { break key; } };
        let outcome = fm.key(key, &mut disk);
        mind::println!("[FM] {}", fm.status());
        if outcome == Outcome::Quit { break; }
    }
    mind::println!("[FM] DONE");
}
