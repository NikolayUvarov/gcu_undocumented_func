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
use fm::fm::{Disk, Failure, Fm, Outcome, Place, Sink, Started, VolumeInfo};
use fm::store::{Store, Volume, WithStore};
use mind::cid::{Cid, Codec};
use mind::dag::{self, Blocks};
use mind::idl::blockstore;
use mind::idl::codec::{List, Text};
use fm::panel::{self, Entry};
use mind::abi::*;
use mind::fs::{self, Error, File};
use mind::idl::loader;
use mind::input::KeyOrPointer;
use mind::ipc::Endpoint;
use mind::tui::viewer::Source;
use mind::tui::{Terminal, CLASSIC};

// The user's files to work on; system information only to lend to the monitors it starts; the block store as the
// volume `store:` (300-APP-0038).
mind::request!(REQUEST_FILES | REQUEST_SYSINFO | REQUEST_BLOCKSTORE);

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

// The block store's client in SLOT_BLOCKSTORE, as fm::store needs it.
#[derive(Clone, Copy)]
struct Remote;

const STORE: Endpoint = Endpoint(SLOT_BLOCKSTORE);

fn fault(error: blockstore::Error) -> Failure {
    match error {
        blockstore::Error::NotFound => Failure::NotFound, blockstore::Error::Full | blockstore::Error::Quota => Failure::NoSpace,
        blockstore::Error::Rights | blockstore::Error::ReadOnly => Failure::Denied, other => Failure::Other(alloc::format!("{:?}", other)),
    }
}
fn lost(_: mind::Error) -> Failure { Failure::Other(String::from("the block store does not answer")) }

impl Blocks for Remote {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> {
        let codec = match codec { Codec::Raw => blockstore::Codec::Raw, Codec::DagCbor => blockstore::Codec::DagCbor };
        let mut out = [0u8; 36];
        match blockstore::put(STORE, codec, data, &mut out) {
            Ok(Ok(n)) => Cid::from_bytes(&out[..n]).map_err(|_| dag::Error::Store),
            Ok(Err(blockstore::Error::Full)) => Err(dag::Error::Full),
            _ => Err(dag::Error::Store),
        }
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        match blockstore::get(STORE, &cid.to_bytes(), out) {
            Ok(Ok(n)) => Ok(n), Ok(Err(blockstore::Error::NotFound)) => Err(dag::Error::NotFound), Ok(Err(blockstore::Error::Corrupt)) => Err(dag::Error::Corrupt), _ => Err(dag::Error::Store),
        }
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { blockstore::has(STORE, &cid.to_bytes()).ok().and_then(Result::ok).ok_or(dag::Error::Store) }
}

impl Store for Remote {
    fn resolve(&mut self, name: &str) -> Result<Option<(u64, Cid)>, String> {
        match blockstore::resolve(STORE, name) {
            Ok(Ok(head)) => Cid::from_bytes(head.root.as_slice()).map(|root| Some((head.version, root))).map_err(|_| String::from("a bad root")),
            Ok(Err(blockstore::Error::NotFound)) => Ok(None),
            Ok(Err(error)) => Err(alloc::format!("{:?}", error)),
            Err(_) => Err(String::from("the block store does not answer")),
        }
    }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<(), Failure> {
        blockstore::publish(STORE, name, expected, &root.to_bytes()).map_err(lost)?.map(drop).map_err(fault)
    }
    fn unpublish(&mut self, name: &str, expected: u64) -> Result<(), Failure> { blockstore::unpublish(STORE, name, expected).map_err(lost)?.map(drop).map_err(fault) }
    fn commit(&mut self, updates: &[(&str, u64, Option<Cid>)]) -> Result<(), Failure> {
        let mut list: Vec<blockstore::Update> = Vec::new();
        for (name, expected, root) in updates {
            let root = root.map_or(List::default(), |r| List::from_slice(&r.to_bytes()).unwrap_or_default());
            list.push(blockstore::Update { name: Text::new(name).ok_or(Failure::Other(String::from("a long name")))?, expected: *expected, root });
        }
        blockstore::commit(STORE, &list).map_err(lost)?.map(drop).map_err(fault)
    }
    fn pins(&mut self) -> Result<Vec<(Cid, u64)>, String> {
        match blockstore::pins(STORE) {
            Ok(Ok(pins)) => Ok(pins.as_slice().iter().filter_map(|p| Cid::from_bytes(p.root.as_slice()).ok().map(|cid| (cid, p.size))).collect()),
            Ok(Err(error)) => Err(alloc::format!("{:?}", error)),
            Err(_) => Err(String::from("the block store does not answer")),
        }
    }
    fn stats(&mut self) -> Result<(u64, u64, u32), String> {
        match blockstore::stat(STORE) { Ok(Ok(s)) => Ok((s.used, s.sectors, s.names)), Ok(Err(error)) => Err(alloc::format!("{:?}", error)), Err(_) => Err(String::from("the block store does not answer")) }
    }
    fn names(&mut self) -> Option<Vec<String>> { None } // blockstore.wit 1.3 has no list of names
}

impl Disk for Vfs {
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        fs::list(path, |e| entries.push(Entry { name: String::from_utf8_lossy(e.name).into(), size: e.size as u64, dir: e.is_dir, flags: e.flags, modified: e.modified }))
            .map_err(|error| alloc::format!("{:?}", error))?;
        Ok(entries)
    }
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>> { File::open(path).ok().map(|file| Box::new(DiskFile(file)) as Box<dyn Source>) }
    // As the shell and wm do: of what the program asks for, what fm holds. In a window of wm fm lends its broker client,
    // so the program opens a window of its own next to fm's instead of a screen in the background (issue 099).
    fn run(&mut self, path: &str, args: &str) -> Result<Started, String> {
        let failed = |e: loader::Error| alloc::format!("{:?}", e);
        let lost = |e: mind::Error| alloc::format!("{:?}", e);
        let needs = loader::inspect(Endpoint::LOADER, path).map_err(lost)?.map_err(failed)?;
        let requests = loader::inspect_requests(Endpoint::LOADER, path).map_err(lost)?.map_err(failed)?;
        let console = mind::process::console_run(requests, args); // `clock --line` too (issue u016)
        // In a window, a console program runs in a window of `console`, which shows what it prints (issue u004).
        if console && mind::windowed::active() {
            let line = alloc::format!("{} {}", path, args);
            return self.run("console", line.trim()).map(|started| Started { place: Place::InConsole, ..started });
        }
        let session = loader::begin(Endpoint::LOADER, path, args).map_err(lost)?.map_err(failed)?;
        let holds = |slot: usize| mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT;
        let lend = |slot: usize| matches!(loader::grant(Endpoint::LOADER, session, slot as u8, slot), Ok(Ok(())));
        let window = !console && mind::windowed::active() && requests & mind::process::REQUEST_WINDOW_MANAGER == 0 && lend(SLOT_WINDOW);
        if needs.files && holds(SLOT_FILE) { lend(SLOT_FILE); }
        if needs.sysinfo && requests & mind::process::REQUEST_AUTHORITY == 0 && holds(SLOT_SYSINFO) { lend(SLOT_SYSINFO); }
        if console || window {
            let pid = loader::commit(Endpoint::LOADER, session).map_err(lost)?.map_err(failed)?;
            return Ok(Started { pid, place: if console { Place::Console } else { Place::Window } });
        }
        // On a screen of its own: in front, as fm is (issue 160); `rights` if fm is not in front, then in the background.
        match loader::commit_in_front(Endpoint::LOADER, session).map_err(lost)? {
            Ok(pid) => Ok(Started { pid, place: Place::Front }),
            Err(loader::Error::Rights) => loader::commit(Endpoint::LOADER, session).map_err(lost)?.map_err(failed).map(|pid| Started { pid, place: Place::Screen }),
            Err(error) => Err(failed(error)),
        }
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
    fn flush(&mut self, path: &str) -> Result<(), Failure> { fs::Dir::root(panel::volume(path).0.trim_end_matches(':')).and_then(|root| root.flush()).map_err(failure) }
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("fm — file manager (Norton Commander keys): two panels over the boot disk (A:), the RAM disk (ram:) and, where mounted, log:, models:\nand the block store (store:: names as files, a copy there publishes one, a delete unpublishes it).\nUsage: fm [directory]\nEnter open or run, F3 view, F4 edit, Shift+F4 new file, F5 copy, F6 move or rename, F7 new directory, F8 delete,\nF9 menu, F1 keys, F10 or Esc quit; Tab other panel, Ins mark, Alt+F1/F2 volume, Alt+F7 find, Ctrl+F3-F6 sort.\nTyping goes to the command line: Enter runs it (cd, edit, view, a program with arguments). Ctrl+O hides the panels,\nCtrl+F1/F2 the left/right one, Ctrl+P the other one. The mouse: a click puts the cursor on an entry, a double click\nopens it, a right click marks it, the wheel scrolls, a click on the key bar presses that key.\nIt may change ram:, log: and data/; other files open read-only. Hold Shift, Ctrl or Alt to see what F1-F10 do with it.");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let Some(mut term) = Terminal::open(info, "fm") else { return };
    let store = (mind::dev::cap_info(SLOT_BLOCKSTORE).0 == CAP_KIND_ENDPOINT).then(|| Volume::new(Remote));
    let mut disk = WithStore { disk: Vfs, store };
    // The viewer's window lives as long as the program; the viewer gives it back when it closes.
    let window: &'static mut [u8] = Box::leak(alloc::vec![0u8; 64 * 1024].into_boxed_slice());
    let mut fm = Fm::new(window, &mut disk);
    // `fm <directory>` opens the left panel there.
    let start = mind::process::args_str().trim().trim_matches('/');
    if !start.is_empty() { fm.load(0, start, None, &mut disk); }
    mind::println!("[FM] READY {}", fm.status());
    // The mouse (issue u001): on a screen the cell under it is shown inverted; in a window wm draws the pointer.
    mind::input::pointer(true);
    term.show_pointer(true);
    loop {
        fm.modifiers = mind::input::modifiers();
        let cursor = { let mut grid = term.grid(); fm.draw(&mut grid, &CLASSIC) };
        term.set_cursor(cursor);
        // In a window (wm, issue 088) the title says where the active panel is.
        term.set_title(&alloc::format!("fm {}", panel::display(&fm.panels[fm.active].path)));
        term.present();
        // While a job runs, keys are only looked at between slices, and the screen is drawn about every 100 ms.
        let input = if fm.busy() {
            let since = mind::time::uptime_ms();
            while fm.busy() && mind::time::uptime_ms() - since < 100 { fm.work(&mut disk); }
            if !fm.busy() { mind::println!("[FM] {}", fm.status()); } // finished, or waiting for an answer
            match mind::input::read_key() { Some(key) => KeyOrPointer::Key(key), None => continue }
        } else {
            // Shift, Ctrl or Alt going down or up changes the key bar: drawn again.
            match mind::input::wait_key_pointer_or_modifiers(fm.modifiers) { Some(input) => input, None => continue }
        };
        let outcome = match input {
            KeyOrPointer::Key(key) => fm.key(key, &mut disk),
            KeyOrPointer::Pointer(p) => {
                let outcome = fm.pointer(p.x, p.y, p.buttons, p.wheel, mind::time::uptime_ms(), &mut disk);
                // Moves of the mouse are not logged.
                if outcome == Outcome::Ignored { continue; }
                mind::println!("[FM] POINTER {},{} BUTTONS={} WHEEL={}", p.x, p.y, p.buttons, p.wheel);
                outcome
            }
        };
        mind::println!("[FM] {}", fm.status());
        if outcome == Outcome::Quit { break; }
    }
    mind::println!("[FM] DONE");
}
