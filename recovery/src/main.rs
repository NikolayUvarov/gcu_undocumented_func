#![no_std]
#![no_main]
// recovery (308-APP-0122): a recovery key of the store's volume key, made by the store, shown once on the screen and
// typed back, or a store that no keeper of this machine opens opened with one (blockstore.wit 2.4, 308-STO-0028), through
// the client badged BADGE_RECOVER that the shell lends in SLOT_RECOVER for REQUEST_RECOVER, asking the user each time
// (308-KRN-0204). The key reaches no log and no serial line: the screen shows it, and without a screen it goes to and
// from the file --key-file names, through the user's files, which it drops at start without that.
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_FILE, SLOT_RECOVER};
use mind::ipc::Endpoint;
use mind::tui::{Terminal, DARK};
use recovery::keyfile::{self, Command};
use recovery::service::{Absent, Service};
use recovery::view::{refusal, Flow, Recovery, Store};

mind::request!(REQUEST_RECOVER | REQUEST_FILES);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("recovery — a recovery key that opens the store's medium on another machine, whose keystore does not keep\nits volume key: made by the store, shown once here and typed back to keep it; or a closed store opened with one.\nUsage: recovery | recovery new --key-file <file> | recovery open --key-file <file>\n↑↓ choose, Enter, m: make, o: open, q or Esc: quit. The key is typed without echo, checked before the store is asked.\nWithout a screen (scripts, tests): new writes the key to <file> (its first line MIND CORE RECOVERY KEY 1, its second\nthe key) and keeps it; open reads it there. The key goes to no log and no serial line.\nThe shell lends the store's recovery client (REQUEST_RECOVER) once you allow it, each time it starts this program.");
    let command = match keyfile::command(mind::process::args_str()) {
        Ok(command) => command,
        Err(usage) => { mind::println!("{}", usage); mind::process::exit_with(2) }
    };
    // The user's files only for the key's file, through the client the shell lent, which writes in data/ (as fm's).
    match command {
        None => { let _ = mind::ipc::drop_cap(SLOT_FILE); }
        Some(_) if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT => mind::fs::use_endpoint(Endpoint(SLOT_FILE)),
        Some(_) => {}
    }
    let held = mind::dev::cap_info(SLOT_RECOVER).0 == CAP_KIND_ENDPOINT;
    let (mut service, mut absent) = (Service(Endpoint(SLOT_RECOVER)), Absent);
    let store: &mut dyn Store = if held { &mut service } else { &mut absent };
    if let Some(command) = command { mind::process::exit_with(without_screen(command, store)) }
    let Some(mut term) = Terminal::open(info, "recovery") else { return };
    let mut view = Recovery::new();
    view.windowed = term.screen().is_none();
    view.refresh(store);
    mind::println!("[RECOVERY] READY: {}", view.status());
    let mut said = String::new();
    loop {
        let cursor = { let mut grid = term.grid(); view.draw(&mut grid, &DARK) };
        term.set_cursor(cursor);
        term.present();
        let Some(key) = mind::input::wait_key(1000) else { continue };
        let before = view.status();
        if let Flow::Quit = view.key(key, store) { break; }
        if view.status() != before { mind::println!("[RECOVERY] {}", view.status()); }
        if view.notice != said { said = view.notice.clone(); if !said.is_empty() { mind::println!("[RECOVERY] {}", said); } }
    }
    mind::println!("[RECOVERY] DONE");
}

// `new` and `open` with the key's file: the lines for the log (never the key), and the exit status: 0 done, 1 refused,
// 3 no client.
fn without_screen(command: Command, store: &mut dyn Store) -> u32 {
    let fail = |text: String| { mind::println!("[RECOVERY] {}", text); 1 };
    match store.records() {
        Err(recovery::view::Refused::NoClient) => { mind::println!("[RECOVERY] {}", refusal("records", recovery::view::Refused::NoClient)); 3 }
        records => match command {
            Command::New(path) => {
                if let Err(r) = records { return fail(alloc::format!("NO KEY MADE: {}", refusal("records", r))); }
                let mut key = match store.make() { Ok(key) => key, Err(r) => return fail(alloc::format!("NO KEY MADE: {}", refusal("make", r))) };
                let mut text = keyfile::render(&key);
                keyfile::wipe(&mut key);
                let written = write(&path, text.as_bytes());
                keyfile::wipe(&mut text);
                if let Err(why) = written { return fail(alloc::format!("NOT KEPT: {} NOT WRITTEN ({}); THE OLDER KEYS STILL OPEN THE MEDIUM", path, why)); }
                // Kept from what the file holds, read back, so the file is known to open the medium.
                let mut back = match read_key(&path) {
                    Ok(Ok(key)) => key,
                    Ok(Err(why)) => return fail(alloc::format!("NOT KEPT: {} READ BACK: {}", path, why)),
                    Err(why) => return fail(alloc::format!("NOT KEPT: {} NOT READ BACK ({})", path, why)),
                };
                let kept = store.keep(&back);
                keyfile::wipe(&mut back);
                match kept {
                    Ok(erased) => { mind::println!("[RECOVERY] A KEY MADE, WRITTEN TO {} AND KEPT: {} OLDER RECOVERY RECORD(S) ERASED", path, erased); 0 }
                    Err(r) => fail(alloc::format!("NOT KEPT: {}", refusal("keep", r))),
                }
            }
            Command::Open(path) => {
                let mut key = match read_key(&path) {
                    Ok(Ok(key)) => key,
                    Ok(Err(why)) => return fail(alloc::format!("NOT OPENED: {}: {}", path, why)),
                    Err(why) => return fail(alloc::format!("NOT OPENED: {} NOT READ ({})", path, why)),
                };
                let opened = store.open(&key);
                keyfile::wipe(&mut key);
                match opened {
                    Ok(()) => { mind::println!("[RECOVERY] THE STORE OPENED WITH THE KEY FROM {}: THIS MACHINE'S KEYSTORE KEEPS ITS VOLUME KEY NOW", path); 0 }
                    Err(r) => fail(alloc::format!("NOT OPENED: {}", refusal("open", r))),
                }
            }
        },
    }
}

fn write(path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut file = mind::fs::File::create(path).map_err(|e| alloc::format!("it cannot be created: {:?}", e))?;
    file.write(bytes).map_err(|e| alloc::format!("it cannot be written: {:?}", e))?;
    file.flush().map_err(|e| alloc::format!("it cannot be flushed: {:?}", e))
}

// The key the file at `path` holds, or why it holds none; what was read is wiped.
fn read_key(path: &str) -> Result<Result<String, String>, String> {
    let mut file = mind::fs::File::open(path).map_err(|e| alloc::format!("it cannot be opened: {:?}", e))?;
    let mut bytes: Vec<u8> = alloc::vec![0u8; file.size().min(512)];
    let mut at = 0;
    while at < bytes.len() { match file.read(&mut bytes[at..]) { Ok(0) => break, Ok(n) => at += n, Err(e) => { keyfile::wipe_bytes(&mut bytes); return Err(alloc::format!("it cannot be read: {:?}", e)) } } }
    bytes.truncate(at);
    let parsed = core::str::from_utf8(&bytes).map(keyfile::parse).map_err(|_| String::from("not text"));
    keyfile::wipe_bytes(&mut bytes);
    parsed
}
