#![no_std]
#![no_main]
// sha256: the SHA-256 of files, as `sha256sum` prints it, to check a file against a manifest (for example a model on
// models:, issue 251). A console program; it reads through its own read-only client.
use mind::abi::BootInfo;
use mind::sha256::Sha256;

mind::request!(REQUEST_CONSOLE);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("sha256 — the SHA-256 of each file, as sha256sum prints it.\nUsage: sha256 <file>...   (e.g. sha256 models:MANIFEST.json)");
    let paths = mind::process::args_str().split_whitespace();
    let mut buffer = [0u8; 8 * 1024];
    let mut any = false;
    for path in paths {
        any = true;
        let mut file = match mind::fs::File::open(path) { Ok(file) => file, Err(error) => { mind::println!("sha256: {}: {:?}", path, error); continue; } };
        let mut hash = Sha256::new();
        let result = loop {
            match file.read(&mut buffer) { Ok(0) => break Ok(()), Ok(n) => hash.update(&buffer[..n]), Err(error) => break Err(error) }
        };
        match result {
            Ok(()) => { let mut hex = [0u8; 64]; for (i, b) in hash.finish().iter().enumerate() { hex[2 * i] = HEX[(b >> 4) as usize]; hex[2 * i + 1] = HEX[(b & 15) as usize]; }
                        mind::println!("{}  {}", core::str::from_utf8(&hex).unwrap_or(""), path); }
            Err(error) => mind::println!("sha256: {}: {:?}", path, error),
        }
    }
    if !any { mind::println!("USAGE: SHA256 <FILE>..."); }
}

const HEX: &[u8; 16] = b"0123456789abcdef";
