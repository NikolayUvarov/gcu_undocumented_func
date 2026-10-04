#![no_std]
#![no_main]
// find: entries of a volume by name mask, type and size (search::find), a console program reading through the
// application's read-only file client.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use search::find::{self, Item, Tree};

mind::request!(REQUEST_CONSOLE);

/// The volumes through `mind::fs`.
pub struct Fs;

impl Tree for Fs {
    fn list(&mut self, path: &str) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        mind::fs::list(path, |e| items.push(Item { name: String::from(e.name_str()), dir: e.is_dir, size: e.size as u64 })).map_err(|error| format!("{:?}", error).to_uppercase())?;
        Ok(items)
    }
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    mind::about!("find — finds entries by name mask, type and size, from a directory down.\nUsage: find [path] [-name mask[,mask]] [-type f|d] [-size +N|-N (bytes, k or M)]");
    let options = match find::parse(mind::process::args_str()) { Ok(options) => options, Err(usage) => { mind::println!("{}", usage); return; } };
    find::walk(&mut Fs, &options, &mut |path, item| mind::println!("{}{}", path, if item.dir { "/" } else { "" }), &mut |path, error| mind::println!("FIND: {}: {}", path, error));
}
