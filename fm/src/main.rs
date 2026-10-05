#![no_std]
#![no_main]
// fm: file manager over vfs_server (fm::fm). It asks its launcher for the user's files (`REQUEST_FILES`): the shell
// lends its own VFS client in SLOT_FILE, so copies, moves, new directories, deletions and the built-in editor can
// change `ram:` and `data/`. Programs it starts go through the loader's launch session with the standard grants and
// run in the background.
extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use fm::fm::{Disk, Failure, Fm, Outcome, Sink, VolumeInfo};
use fm::panel::{self, Entry};
use mind::abi::*;
use mind::fs::{self, Error, File};
use mind::idl::loader;
use mind::ipc::Endpoint;
use mind::tui::viewer::Source;
use mind::tui::{Terminal, CLASSIC};

mind::request!(REQUEST_FILES);

struct DiskFile(File);
impl Source for DiskFile {
    fn size(&self) -> u64 { self.0.size() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { self.0.read_at(offset as usize, out).unwrap_or(0) }
}

struct DiskSink(File);
impl Sink for DiskSink {
    fn write(&mut self, data: &[u8]) -> Result<(), Failure> { self.0.write(data).map(drop).map_err(failure) }
}

fn failure(error: Error) -> Failure {
    match error {
        Error::Exists => Failure::Exists, Error::NotEmpty => Failure::NotEmpty, Error::Denied | Error::ReadOnly => Failure::Denied,
        Error::NoSpace => Failure::NoSpace, Error::NotFound => Failure::NotFound, other => Failure::Other(alloc::format!("{:?}", other)),
    }
}

struct Vfs;

impl Disk for Vfs {
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        fs::list(path, |e| entries.push(Entry { name: String::from_utf8_lossy(e.name).into(), size: e.size as u64, dir: e.is_dir, flags: e.flags, modified: e.modified }))
            .map_err(|error| alloc::format!("{:?}", error))?;
        Ok(entries)
    }
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>> { File::open(path).ok().map(|file| Box::new(DiskFile(file)) as Box<dyn Source>) }
    fn run(&mut self, path: &str, args: &str) -> Result<u64, String> {
        let failed = |e: loader::Error| alloc::format!("{:?}", e);
        let session = loader::begin(Endpoint::LOADER, path, args).map_err(|e| alloc::format!("{:?}", e))?.map_err(failed)?;
        loader::commit(Endpoint::LOADER, session).map_err(|e| alloc::format!("{:?}", e))?.map_err(failed)
    }
    fn create(&mut self, path: &str, replace: bool) -> Result<Box<dyn Sink>, Failure> {
        let mode = fs::MODE_WRITE | fs::MODE_CREATE | if replace { fs::MODE_TRUNCATE } else { fs::MODE_NEW };
        File::open_mode(path, mode).map(|file| Box::new(DiskSink(file)) as Box<dyn Sink>).map_err(failure)
    }
    fn mkdir(&mut self, path: &str) -> Result<(), Failure> { fs::mkdir(path).map_err(failure) }
    fn remove(&mut self, path: &str) -> Result<(), Failure> { fs::remove(path).map_err(failure) }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), Failure> { fs::rename(from, to).map_err(failure) }
    fn writable(&mut self, path: &str) -> bool { File::open_mode(path, fs::MODE_WRITE).is_ok() }
    fn volume(&mut self, path: &str) -> Option<VolumeInfo> {
        let name = panel::volume(path).0.trim_end_matches(':');
        fs::volume(name).ok().map(|v| VolumeInfo { label: String::from(v.label()), fat_bits: v.fat_bits, bytes: v.bytes, free: v.free })
    }
    fn flush(&mut self, path: &str) { if let Ok(root) = fs::Dir::root(panel::volume(path).0.trim_end_matches(':')) { let _ = root.flush(); } }
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("fm — file manager (Norton Commander keys): two panels over the boot disk (A:) and the RAM disk (ram:).\nUsage: fm [directory]\nEnter open or run, F3 view, F4 edit, Shift+F4 new file, F5 copy, F6 move or rename, F7 new directory, F8 delete,\nF9 menu, F1 keys, F10 or Esc quit; Tab other panel, Ins mark, Alt+F1/F2 volume, Alt+F7 find, Ctrl+F3-F6 sort.\nTyping goes to the command line: Enter runs it (cd, edit, view, a program with arguments). Ctrl+O hides the panels,\nCtrl+F1/F2 the left/right one, Ctrl+P the other one.\nIt may change ram: and data/; other files open read-only. Hold Shift, Ctrl or Alt to see what F1-F10 do with it.");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let Some(mut term) = Terminal::open(info, "fm") else { return };
    let mut disk = Vfs;
    // The viewer's window lives as long as the program; the viewer gives it back when it closes.
    let window: &'static mut [u8] = Box::leak(alloc::vec![0u8; 64 * 1024].into_boxed_slice());
    let mut fm = Fm::new(window, &mut disk);
    // `fm <directory>` opens the left panel there.
    let start = mind::process::args_str().trim().trim_matches('/');
    if !start.is_empty() { fm.load(0, start, None, &mut disk); }
    mind::println!("[FM] READY {}", fm.status());
    loop {
        fm.modifiers = mind::input::modifiers();
        let cursor = { let mut grid = term.grid(); fm.draw(&mut grid, &CLASSIC) };
        term.set_cursor(cursor);
        // In a window (wm, issue 088) the title says where the active panel is.
        term.set_title(&alloc::format!("fm {}", panel::display(&fm.panels[fm.active].path)));
        term.present();
        // While a job runs, keys are only looked at between slices, and the screen is drawn about every 100 ms.
        let key = if fm.busy() {
            let since = mind::time::uptime_ms();
            while fm.busy() && mind::time::uptime_ms() - since < 100 { fm.work(&mut disk); }
            if !fm.busy() { mind::println!("[FM] {}", fm.status()); } // finished, or waiting for an answer
            match mind::input::read_key() { Some(key) => key, None => continue }
        } else {
            // Shift, Ctrl or Alt going down or up changes the key bar: drawn again.
            match mind::input::wait_key_or_modifiers(fm.modifiers) { Some(key) => key, None => continue }
        };
        let outcome = fm.key(key, &mut disk);
        mind::println!("[FM] {}", fm.status());
        if outcome == Outcome::Quit { break; }
    }
    mind::println!("[FM] DONE");
}
