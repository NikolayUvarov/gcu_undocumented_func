//! `find [path] [-name masks] [-type f|d] [-size +N|-N]`: walks a directory tree depth-first and in name order. Paths
//! are written as the shell takes them: `docs/a.txt` on the boot disk (`A:` may be given for its root), `ram:docs/a.txt`
//! on the RAM disk.
use crate::pattern;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// An entry of a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item { pub name: String, pub dir: bool, pub size: u64 }

/// A directory tree: the program reads it through `mind::fs`, tests from memory.
pub trait Tree {
    /// The entries of `path` (without `.` and `..`), or why it cannot be read.
    fn list(&mut self, path: &str) -> Result<Vec<Item>, String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size { Larger(u64), Smaller(u64) }

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options { pub root: String, pub name: Option<String>, pub dirs: Option<bool>, pub size: Option<Size> }

pub const USAGE: &str = "USAGE: FIND [PATH] [-name MASK[,MASK]] [-type f|d] [-size +N|-N (bytes, k or M)]";

/// Directories deeper than this are not entered (FAT has no links, so a walk always ends; this bounds the work).
pub const MAX_DEPTH: usize = 32;

/// `A:`, `a:` and nothing name the boot disk's root.
pub fn normalize(path: &str) -> String {
    let path = path.trim_end_matches('/');
    let rest = path.strip_prefix("A:").or_else(|| path.strip_prefix("a:")).unwrap_or(path);
    String::from(rest.trim_start_matches('/'))
}

/// `dir` and `name` as one path: `ram:` + `a` = `ram:a`, `ram:docs` + `a` = `ram:docs/a`, `` + `a` = `a`.
pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() || dir.ends_with(':') || dir.ends_with('/') { format!("{}{}", dir, name) } else { format!("{}/{}", dir, name) }
}

fn number(text: &str) -> Option<u64> {
    let (digits, unit) = match text.char_indices().last() { Some((i, 'k' | 'K')) => (&text[..i], 1024), Some((i, 'm' | 'M')) => (&text[..i], 1024 * 1024), _ => (text, 1) };
    digits.parse::<u64>().ok().and_then(|n| n.checked_mul(unit))
}

pub fn parse(args: &str) -> Result<Options, String> {
    let mut options = Options::default();
    let mut words = args.split_whitespace();
    let mut root = None;
    while let Some(word) = words.next() {
        match word {
            "-name" | "-iname" => options.name = Some(String::from(words.next().ok_or_else(|| String::from(USAGE))?)),
            "-type" => options.dirs = Some(match words.next() { Some("d") => true, Some("f") => false, _ => return Err(String::from(USAGE)) }),
            "-size" => {
                let value = words.next().ok_or_else(|| String::from(USAGE))?;
                options.size = Some(match value.as_bytes().first() {
                    Some(b'+') => Size::Larger(number(&value[1..]).ok_or_else(|| String::from(USAGE))?),
                    Some(b'-') => Size::Smaller(number(&value[1..]).ok_or_else(|| String::from(USAGE))?),
                    _ => return Err(String::from(USAGE)),
                });
            }
            option if option.starts_with('-') => return Err(format!("FIND: UNKNOWN OPTION {}\n{}", option, USAGE)),
            path if root.is_none() => root = Some(normalize(path)),
            _ => return Err(String::from(USAGE)),
        }
    }
    options.root = root.unwrap_or_default();
    Ok(options)
}

/// Whether an entry is reported.
pub fn selected(options: &Options, item: &Item) -> bool {
    if options.dirs.is_some_and(|dirs| dirs != item.dir) { return false; }
    if let Some(masks) = &options.name { if !pattern::matches(masks, &item.name) { return false; } }
    match options.size {
        Some(Size::Larger(n)) => !item.dir && item.size > n,
        Some(Size::Smaller(n)) => !item.dir && item.size < n,
        None => true,
    }
}

/// Lists the tree under `options.root` depth-first, each directory before its contents, names in order (case-insensitive).
/// `found` gets the path and entry of every selected entry, `failed` a path that could not be read and why. Returns
/// the number of entries found.
pub fn walk(tree: &mut dyn Tree, options: &Options, found: &mut dyn FnMut(&str, &Item), failed: &mut dyn FnMut(&str, &str)) -> usize {
    let mut count = 0;
    // Pending entries, the next one last: (path, entry, depth).
    let mut stack: Vec<(String, Item, usize)> = Vec::new();
    let push = |stack: &mut Vec<(String, Item, usize)>, dir: &str, depth: usize, failed: &mut dyn FnMut(&str, &str), tree: &mut dyn Tree| {
        match tree.list(dir) {
            Ok(mut items) => {
                items.retain(|i| i.name != "." && i.name != "..");
                items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                for item in items.into_iter().rev() { stack.push((join(dir, &item.name), item, depth)); }
            }
            Err(error) => failed(if dir.is_empty() { "A:" } else { dir }, &error),
        }
    };
    push(&mut stack, &options.root, 1, failed, tree);
    while let Some((path, item, depth)) = stack.pop() {
        if selected(options, &item) { found(&path, &item); count += 1; }
        if item.dir && depth < MAX_DEPTH { push(&mut stack, &path, depth + 1, failed, tree); }
    }
    count
}
