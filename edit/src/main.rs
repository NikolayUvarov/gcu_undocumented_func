#![no_std]
#![no_main]
// edit: text editor (edit::editor). It asks its launcher for a file capability (`REQUEST_FILE`): the shell lends its
// own VFS client in SLOT_FILE, so the editor can save where the user may write (`ram:`, `data/`). Without it the
// editor has the read-only client every application gets, and a file opens read-only.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use edit::buffer::LIMIT;
use edit::editor::{Editor, Outcome};
use mind::abi::*;
use mind::fs::{self, Dir, Error, File};
use mind::gfx::Screen;
use mind::ipc::Endpoint;
use mind::tui::{Terminal, CLASSIC};

mind::request!(REQUEST_FILE);
mind::entry!(main);

fn describe(error: Error) -> String {
    match error {
        Error::Denied | Error::ReadOnly => String::from("denied (only ram: and data/ are writable)"),
        Error::NoSpace => String::from("the disk is full"),
        Error::NotFound => String::from("no such directory"),
        Error::Name | Error::Invalid => String::from("invalid name"),
        other => format!("{:?}", other),
    }
}

// The file's text and whether it may be changed in place; a missing file is a new, empty one.
fn load(path: &str) -> Result<(Vec<u8>, bool), String> {
    if path.is_empty() { return Ok((Vec::new(), false)); }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(Error::NotFound) => return Ok((Vec::new(), false)),
        Err(error) => return Err(format!("{}: {}", path, describe(error))),
    };
    if file.size() > LIMIT { return Err(format!("{}: larger than {} MiB", path, LIMIT >> 20)); }
    let mut text = vec![0u8; file.size()];
    let got = file.read_at(0, &mut text).map_err(|error| format!("{}: {}", path, describe(error)))?;
    text.truncate(got);
    let read_only = File::open_mode(path, VFS_MODE_WRITE).is_err();
    Ok((text, read_only))
}

// Writes `name.tmp`, flushes it, then puts it in place of `path` (FAT has no atomic replace: if the rename fails after
// the old file is gone, the text is in `name.tmp`).
fn save(path: &str, text: &[u8]) -> Result<usize, String> {
    let temporary = format!("{}.tmp", path);
    let written = (|| -> Result<(), Error> {
        let mut file = File::create(&temporary)?;
        file.write_at(0, text)?;
        file.flush()
    })();
    if let Err(error) = written { let _ = fs::remove(&temporary); return Err(describe(error)); }
    match fs::remove(path) {
        Ok(()) | Err(Error::NotFound) => {}
        Err(error) => { let _ = fs::remove(&temporary); return Err(describe(error)); }
    }
    fs::rename(&temporary, path).map_err(|error| format!("{} (the text is in {})", describe(error), temporary))?;
    Dir::root(fs::split(path).0).and_then(|root| root.flush()).map_err(describe)?;
    Ok(text.len())
}

fn main(info: &'static mind::BootInfo) {
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let path = mind::process::args_str().trim();
    let (text, read_only) = match load(path) {
        Ok(loaded) => loaded,
        Err(error) => { mind::println!("[EDIT] CANNOT OPEN {}", error); return; }
    };
    let Some(mut term) = Screen::new(info).and_then(Terminal::new) else { return };
    let mut editor = Editor::new(text, path, read_only);
    mind::println!("[EDIT] READY {} RO={}", editor.status(), read_only as u8);
    loop {
        let cursor = { let mut grid = term.grid(); editor.draw(&mut grid, &CLASSIC) };
        term.set_cursor(cursor);
        term.present();
        let key = loop { if let Some(key) = mind::input::wait_key(1000) { break key; } };
        let (path, quit) = match editor.key(key) {
            Outcome::Quit => break,
            Outcome::Save(path) => (path, false),
            Outcome::SaveAndQuit(path) => (path, true),
            Outcome::Redraw | Outcome::Ignored => { mind::println!("[EDIT] {}", editor.status()); continue; }
        };
        let result = save(&path, &editor.buffer.text());
        match &result { Ok(bytes) => mind::println!("[EDIT] SAVED {} BYTES TO {}", bytes, path), Err(error) => mind::println!("[EDIT] NOT SAVED {}", error) }
        let saved = result.is_ok();
        editor.saved(&path, result);
        mind::println!("[EDIT] {}", editor.status());
        if quit && saved { break; }
    }
    mind::println!("[EDIT] DONE");
}
