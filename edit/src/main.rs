#![no_std]
#![no_main]
// edit: text editor (edit::editor). It asks its launcher for its file (`REQUEST_FILE`): the shell lends a VFS client
// confined to the file's directory (`ram:` without a file), writable where the user may write, in SLOT_FILE; the
// editor keeps using full paths (`mind::fs::use_scope`). Without it the editor has the read-only client every
// application gets, and a file opens read-only.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use edit::buffer::LIMIT;
use edit::editor::{Editor, Outcome};
use mind::abi::*;
use mind::fs::{self, Dir, Error, File};
use mind::ipc::Endpoint;
use mind::tui::{Terminal, CLASSIC};

mind::request!(REQUEST_FILE);
mind::entry!(main);

fn describe(error: Error) -> String {
    match error {
        Error::Denied | Error::ReadOnly => String::from("denied: the editor may change only its file's directory, where you may"),
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
    let read_only = File::open_mode(path, fs::MODE_WRITE).is_err();
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
    mind::about!("edit — text editor: UTF-8 text in Russian and English, up to 8 MiB.\nUsage: edit [file]   (without a file: a new one on ram:)\nF1 keys, F2 save, Shift+F2 save as, F7 find, Shift+F7 next, Ctrl+F7 replace, Alt+F8 go to line, F9 menu, F10 or Esc quit.\nFiles on ram: and in data/ can be changed, others open read-only. Hold Shift, Ctrl or Alt to see what F1-F10 do with it.");
    let path = mind::process::args_str().trim();
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT {
        // The client's root is the file's directory.
        let base = if path.is_empty() { String::from("ram:") } else {
            let (volume, rest) = fs::split(path);
            let parent = rest.rfind('/').map_or("", |i| &rest[..i]);
            if volume.is_empty() { String::from(parent) } else { format!("{}:{}", volume, parent) }
        };
        fs::use_scope(Endpoint(SLOT_FILE), &base);
    }
    let (text, read_only) = match load(path) {
        Ok(loaded) => loaded,
        Err(error) => { mind::println!("[EDIT] CANNOT OPEN {}", error); return; }
    };
    let Some(mut term) = Terminal::open(info, "edit") else { return };
    let mut editor = Editor::new(text, path, read_only);
    mind::println!("[EDIT] READY {} RO={}", editor.status(), read_only as u8);
    loop {
        editor.modifiers = mind::input::modifiers();
        let cursor = { let mut grid = term.grid(); editor.draw(&mut grid, &CLASSIC) };
        term.set_cursor(cursor);
        term.present();
        // Shift, Ctrl or Alt going down or up changes the key bar: drawn again.
        let Some(key) = mind::input::wait_key_or_modifiers(editor.modifiers) else { continue };
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
