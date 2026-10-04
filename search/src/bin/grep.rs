#![no_std]
#![no_main]
// grep: lines of text files that match a simple regular expression (search::grep), a console program reading through
// the application's read-only file client.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use search::find::{Item, Tree};
use search::grep;

mind::request!(REQUEST_CONSOLE);

struct Fs;

impl Tree for Fs {
    fn list(&mut self, path: &str) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        mind::fs::list(path, |e| items.push(Item { name: String::from(e.name_str()), dir: e.is_dir, size: e.size as u64 })).map_err(|error| format!("{:?}", error).to_uppercase())?;
        Ok(items)
    }
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    mind::about!("grep — prints the lines of text files that match a pattern (. * [a-z] [^...] ^ $ \\).\nUsage: grep [-i] [-n] [-r] [-l] [-c] pattern [path...]\n-i ignore case, -n line numbers, -r into directories, -l file names only, -c counts only.");
    let options = match grep::parse(mind::process::args_str()) { Ok(options) => options, Err(usage) => { mind::println!("{}", usage); return; } };
    let files = grep::files(&mut Fs, &options, &mut |path, error| mind::println!("GREP: {}: {}", path, error));
    let several = files.len() > 1 || options.recursive;
    for path in &files {
        let mut file = match mind::fs::File::open(path) { Ok(file) => file, Err(error) => { mind::println!("GREP: {}: {:?}", path, error); continue; } };
        let mut read = |buffer: &mut [u8]| file.read(buffer).map_err(|error| format!("{:?}", error));
        let quiet = options.names_only || options.count;
        let scan = grep::scan(&mut read, &options.pattern, &mut |number, text| if !quiet { mind::println!("{}", grep::line(&options, several, path, number, text)); });
        match scan {
            Ok(scan) if options.count => mind::println!("{}{}", if several { format!("{}:", path) } else { String::new() }, scan.matched),
            Ok(scan) if scan.matched > 0 && options.names_only => mind::println!("{}", path),
            Ok(scan) if scan.matched > 0 && scan.binary => mind::println!("BINARY FILE {} MATCHES", path),
            Ok(_) => {}
            Err(error) => mind::println!("GREP: {}: {}", path, error),
        }
    }
}
