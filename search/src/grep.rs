//! `grep [-i] [-n] [-r] [-l] [-c] pattern [path...]`: lines that match a simple regular expression
//! (`mind::pattern::Pattern`). Files are read in chunks; a line longer than `MAX_LINE` bytes is matched in pieces of
//! that size. A file with a NUL byte is binary: its lines are not printed, only that it matches.
use crate::find::{self, Tree};
use crate::pattern::Pattern;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub const MAX_LINE: usize = 4096;

pub const USAGE: &str = "USAGE: GREP [-i] [-n] [-r] [-l] [-c] PATTERN [PATH...]  (. * [a-z] [^...] ^ $ \\)";

#[derive(Clone, Debug)]
pub struct Options {
    pub pattern: Pattern,
    pub ignore_case: bool,
    pub numbers: bool,
    pub recursive: bool,
    pub names_only: bool,
    pub count: bool,
    pub paths: Vec<String>,
}

pub fn parse(args: &str) -> Result<Options, String> {
    let (mut ignore_case, mut numbers, mut recursive, mut names_only, mut count) = (false, false, false, false, false);
    let mut words = args.split_whitespace().peekable();
    while let Some(word) = words.peek().copied().filter(|w| w.len() > 1 && w.starts_with('-')) {
        words.next();
        for flag in word[1..].chars() {
            match flag { 'i' => ignore_case = true, 'n' => numbers = true, 'r' | 'R' => recursive = true, 'l' => names_only = true, 'c' => count = true,
                         _ => return Err(format!("GREP: UNKNOWN OPTION -{}\n{}", flag, USAGE)) }
        }
    }
    let text = words.next().ok_or_else(|| String::from(USAGE))?;
    let pattern = Pattern::new(text, ignore_case).map_err(|error| format!("GREP: BAD PATTERN ({:?})", error))?;
    let mut paths: Vec<String> = words.map(find::normalize).collect();
    if paths.is_empty() {
        if !recursive { return Err(String::from(USAGE)); }
        paths.push(String::new()); // -r without a path: the boot disk
    }
    Ok(Options { pattern, ignore_case, numbers, recursive, names_only, count, paths })
}

/// What scanning one file found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scan { pub lines: usize, pub matched: usize, pub binary: bool }

/// Scans a stream (`read` fills a buffer and returns the bytes read, 0 at the end): `hit` gets each matching line with
/// its number (from 1). A NUL byte in the first chunk makes the stream binary: its lines are counted, not passed on.
pub fn scan(read: &mut dyn FnMut(&mut [u8]) -> Result<usize, String>, pattern: &Pattern, hit: &mut dyn FnMut(usize, &str)) -> Result<Scan, String> {
    let mut chunk = [0u8; 4096];
    let mut line: Vec<u8> = Vec::with_capacity(256);
    let mut result = Scan::default();
    let mut first = true;
    let test = |line: &mut Vec<u8>, result: &mut Scan, hit: &mut dyn FnMut(usize, &str)| {
        result.lines += 1;
        if line.last() == Some(&b'\r') { line.pop(); }
        let text = String::from_utf8_lossy(line);
        if pattern.is_match(&text) {
            result.matched += 1;
            if !result.binary { hit(result.lines, &text); }
        }
        line.clear();
    };
    loop {
        let got = read(&mut chunk)?;
        if got == 0 { break; }
        if first { result.binary = chunk[..got].contains(&0); first = false; }
        for &byte in &chunk[..got] {
            if byte == b'\n' { test(&mut line, &mut result, hit); continue; }
            line.push(byte);
            if line.len() >= MAX_LINE { test(&mut line, &mut result, hit); }
        }
    }
    if !line.is_empty() { test(&mut line, &mut result, hit); }
    Ok(result)
}

/// The files to scan: the given paths, and with `-r` the files below the directories among them (in `find`'s order).
pub fn files(tree: &mut dyn Tree, options: &Options, failed: &mut dyn FnMut(&str, &str)) -> Vec<String> {
    let mut out = Vec::new();
    for path in &options.paths {
        let is_dir = path.is_empty() || path.ends_with(':') || tree.list(path).is_ok();
        if is_dir {
            if !options.recursive { failed(path, "IS A DIRECTORY (USE -R)"); continue; }
            let walk = find::Options { root: path.clone(), dirs: Some(false), ..find::Options::default() };
            find::walk(tree, &walk, &mut |file, _| out.push(String::from(file)), failed);
        } else {
            out.push(path.clone());
        }
    }
    out
}

/// One output line: the path (when several files are searched), the line number (`-n`) and the text.
pub fn line(options: &Options, several: bool, path: &str, number: usize, text: &str) -> String {
    let mut out = String::new();
    if several { out.push_str(path); out.push(':'); }
    if options.numbers { out.push_str(&format!("{}:", number)); }
    out.push_str(text);
    out
}
