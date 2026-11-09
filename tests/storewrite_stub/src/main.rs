#![no_std]
#![no_main]
//! Test-only console program (QEMU `shell` suite, 311-APP-0121): it asks for no file, so the shell lends it, for the
//! store volume its argument names, a read-only scope of that volume in SLOT_STOREFS. Through mind::fs it reads the
//! path's directory and tries to write the path; it prints what each answered. Never packaged into the normal OS.
use mind::abi::BootInfo;
use mind::fs::{self, File, MODE_CREATE, MODE_WRITE};

mind::request!(REQUEST_CONSOLE);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let path = mind::process::args_str().trim();
    if path.is_empty() { mind::println!("USAGE: STOREWRITE <PATH ON A STORE VOLUME>"); mind::process::exit_with(2) }
    // The path's directory: before its last `/`, or the volume's root.
    let volume_root = &path[..path.find(':').map_or(0, |i| i + 1)];
    let dir = path.rsplit_once('/').map_or(volume_root, |(dir, _)| dir);
    match fs::list(dir, |_| {}) {
        Ok(n) => mind::println!("STOREWRITE READ {}: {} ENTRIES", dir, n),
        Err(e) => mind::println!("STOREWRITE READ {}: {:?}", dir, e),
    }
    match File::open_mode(path, MODE_WRITE | MODE_CREATE) {
        Ok(_) => mind::println!("STOREWRITE WRITE {} ALLOWED", path),
        Err(e) => mind::println!("STOREWRITE WRITE {} REFUSED: {:?}", path, e),
    }
    mind::process::exit_with(0)
}
