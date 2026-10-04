#![no_std]
#![no_main]
// format: a new empty FAT volume on the RAM disk (vfs.wit 2.3 `format`, run by vfs_server), confirmed by -y. It asks
// for the user's files (REQUEST_FILES): only the user's client may format, and only `ram:` can be formatted.
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_FILE};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES);

const USAGE: &str = "USAGE: FORMAT RAM: [-l LABEL] [-y]";

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("format — a new, empty file system on the RAM disk: everything on it is erased.\nUsage: format ram: [-l label] -y   (without -y it only says what it would do)");
    let (mut volume, mut label, mut yes) = (None, "MIND RAM", false);
    let mut words = mind::process::args_str().split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "-y" => yes = true,
            "-l" => match words.next() { Some(text) if text.len() <= 11 => label = text, _ => { mind::println!("FORMAT: A LABEL HAS AT MOST 11 CHARACTERS"); return; } },
            "ram:" | "ram" | "RAM:" => volume = Some("ram"),
            other => { mind::println!("FORMAT: ONLY THE RAM DISK (ram:) CAN BE FORMATTED, NOT {}\n{}", other, USAGE); return; }
        }
    }
    if volume.is_none() { mind::println!("{}", USAGE); return; }
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    // A console program gets no keys (the shell keeps them), so the confirmation is the -y on the command line.
    if !yes {
        mind::println!("FORMAT: THIS ERASES ALL FILES ON ram: (NOTHING WAS CHANGED). TO GO ON: format ram: -l {} -y", label);
        return;
    }
    match mind::fs::Dir::root("ram").and_then(|root| root.format(label)) {
        Ok(()) => {
            let mut upper = [0u8; 11];
            for (out, byte) in upper.iter_mut().zip(label.bytes()) { *out = byte.to_ascii_uppercase(); }
            mind::println!("FORMATTED ram: AS {}", core::str::from_utf8(&upper[..label.len()]).unwrap_or(label));
        }
        Err(error) => mind::println!("FORMAT: {:?}", error),
    }
}
