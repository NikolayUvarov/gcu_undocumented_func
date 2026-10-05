//! Host tests of `find` and `grep` (search/src) and of the shared patterns (libmind/src/pattern.rs): masks, simple
//! regular expressions with case folding, the walk over a tree in memory, line scanning.
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/mask.rs"]
mod mask;
#[path = "../libmind/src/pattern.rs"]
mod pattern;
#[path = "../search/src/find.rs"]
mod find;
#[path = "../search/src/grep.rs"]
mod grep;

use find::{Item, Size, Tree};
use pattern::{Pattern, PatternError};
use std::collections::BTreeMap;

fn re(text: &str, fold: bool) -> Pattern { Pattern::new(text, fold).unwrap() }

#[test]
fn masks() {
    assert!(pattern::matches("*.txt", "Notes.TXT"));
    assert!(pattern::matches("*.rs, *.toml", "Cargo.toml"));
    assert!(pattern::matches("*.*", "README"));
    assert!(pattern::matches("з?метки*", "Заметки.txt"), "Cyrillic folds case");
    assert!(!pattern::matches("*.txt", "notes.md"));
}

#[test]
fn masks_without_allocation() {
    // mind::mask works on characters with byte offsets (the shell, which has no heap, uses it for `list a*`).
    assert!(mask::glob("a*", "app2") && mask::glob("A*", "app") && !mask::glob("a*", "beep"));
    assert!(mask::glob("*mon*", "sysmon") && mask::glob("*", "") && !mask::glob("?", ""));
    assert!(mask::glob("f?", "fm") && !mask::glob("f?", "fsck") && mask::glob("f*k", "fsck"));
    assert!(mask::glob("*ки", "заметки") && mask::glob("?а*", "Заметки") && !mask::glob("*ки?", "заметки"));
    assert!(mask::glob("*a*b", "xaab") && mask::glob("**", "anything") && !mask::glob("a*b", "acbc"));
    assert!(mask::matches("x* a*", "app") && !mask::matches("x*, y*", "app"));
}

#[test]
fn regular_expressions() {
    assert!(re("hello", false).is_match("say hello world"));
    assert!(!re("Hello", false).is_match("say hello"));
    assert!(re("Hello", true).is_match("say hello"));
    assert!(re("ПРИВЕТ", true).is_match("— привет, мир"), "Cyrillic folds case");
    assert!(re("^say", false).is_match("say it") && !re("^say", false).is_match("I say"));
    assert!(re("end$", false).is_match("the end") && !re("end$", false).is_match("ends"));
    assert!(re("h.llo", false).is_match("hallo") && re("ab*c", false).is_match("ac") && re("ab*c", false).is_match("abbbc"));
    assert!(re("[0-9][0-9]*", false).is_match("issue 080") && !re("^[0-9]", false).is_match("issue"));
    assert!(re("[^a-z]", false).is_match("abc1") && !re("^[^a-z]*$", false).is_match("abc"));
    assert!(re("a\\.b", false).is_match("a.b") && !re("a\\.b", false).is_match("axb"));
    assert!(re("^$", false).is_match("") && !re("^$", false).is_match("x"));
    assert!(re(".*", false).is_match(""));
    assert_eq!(Pattern::new("[abc", false).unwrap_err(), PatternError::UnclosedClass);
    assert_eq!(Pattern::new("*a", false).unwrap_err(), PatternError::NothingToRepeat);
    assert_eq!(Pattern::new("a\\", false).unwrap_err(), PatternError::TrailingBackslash);
}

/// A tree in memory: directory path -> entries.
struct Memory(BTreeMap<String, Vec<Item>>);

impl Tree for Memory {
    fn list(&mut self, path: &str) -> Result<Vec<Item>, String> { self.0.get(path).cloned().ok_or_else(|| String::from("NOT FOUND")) }
}

fn item(name: &str, dir: bool, size: u64) -> Item { Item { name: name.into(), dir, size } }

fn disk() -> Memory {
    let mut tree = BTreeMap::new();
    tree.insert(String::from("ram:"), vec![item("notes.txt", false, 300), item("docs", true, 0), item("Заметки.TXT", false, 5000), item(".", true, 0)]);
    tree.insert(String::from("ram:docs"), vec![item("b.md", false, 10), item("a.txt", false, 2048), item("old", true, 0), item("..", true, 0)]);
    tree.insert(String::from("ram:docs/old"), vec![item("c.txt", false, 1)]);
    tree.insert(String::new(), vec![item("kernel.elf", false, 900_000), item("EFI", true, 0)]);
    tree.insert(String::from("EFI"), vec![]);
    Memory(tree)
}

fn find(args: &str) -> (Vec<String>, Vec<String>) {
    let options = find::parse(args).unwrap();
    let (mut found, mut failed) = (Vec::new(), Vec::new());
    find::walk(&mut disk(), &options, &mut |path, _| found.push(path.to_string()), &mut |path, error| failed.push(format!("{}: {}", path, error)));
    (found, failed)
}

#[test]
fn find_walks_in_order_with_filters() {
    let (all, failed) = find("ram:");
    assert_eq!(all, ["ram:docs", "ram:docs/a.txt", "ram:docs/b.md", "ram:docs/old", "ram:docs/old/c.txt", "ram:notes.txt", "ram:Заметки.TXT"]);
    assert!(failed.is_empty());
    assert_eq!(find("ram: -name *.txt").0, ["ram:docs/a.txt", "ram:docs/old/c.txt", "ram:notes.txt", "ram:Заметки.TXT"]);
    assert_eq!(find("ram: -type d").0, ["ram:docs", "ram:docs/old"]);
    assert_eq!(find("ram: -size +1k").0, ["ram:docs/a.txt", "ram:Заметки.TXT"]);
    assert_eq!(find("ram: -size -100 -type f").0, ["ram:docs/b.md", "ram:docs/old/c.txt"]);
    assert_eq!(find("A: -name *.elf").0, ["kernel.elf"], "A: is the boot disk's root");
    assert_eq!(find("").0, ["EFI", "kernel.elf"]);
    let (none, failed) = find("ram:missing");
    assert!(none.is_empty() && failed == ["ram:missing: NOT FOUND"], "{:?}", failed);
    assert_eq!(find::parse("-size +2M").unwrap().size, Some(Size::Larger(2 << 20)));
    assert!(find::parse("-type x").is_err() && find::parse("-bogus").is_err() && find::parse("a b").is_err());
    assert_eq!(find::join("ram:", "a"), "ram:a");
    assert_eq!(find::join("ram:docs", "a"), "ram:docs/a");
    assert_eq!(find::join("", "a"), "a");
}

fn stream(data: &[u8]) -> impl FnMut(&mut [u8]) -> Result<usize, String> + '_ {
    let mut at = 0;
    move |buffer: &mut [u8]| { let n = buffer.len().min(data.len() - at).min(7); buffer[..n].copy_from_slice(&data[at..at + n]); at += n; Ok(n) }
}

#[test]
fn grep_scans_lines() {
    let text = "first line\r\nПривет, мир\nsecond line\nno newline at the end: line";
    let options = grep::parse("-in LINE ram:t.txt").unwrap();
    let mut hits = Vec::new();
    let scan = grep::scan(&mut stream(text.as_bytes()), &options.pattern, &mut |n, line| hits.push(grep::line(&options, false, "ram:t.txt", n, line))).unwrap();
    assert_eq!(hits, ["1:first line", "3:second line", "4:no newline at the end: line"], "chunks of 7 bytes split lines and characters");
    assert_eq!(scan, grep::Scan { lines: 4, matched: 3, binary: false });
    let options = grep::parse("привет ram:t.txt").unwrap();
    let mut hits = Vec::new();
    grep::scan(&mut stream(text.as_bytes()), &options.pattern, &mut |n, line| hits.push(grep::line(&options, true, "ram:t.txt", n, line))).unwrap();
    assert!(hits.is_empty(), "without -i the case must match");
    let options = grep::parse("-i привет ram:t.txt").unwrap();
    grep::scan(&mut stream(text.as_bytes()), &options.pattern, &mut |n, line| hits.push(grep::line(&options, true, "ram:t.txt", n, line))).unwrap();
    assert_eq!(hits, ["ram:t.txt:Привет, мир"]);
    // Binary: counted, not printed.
    let mut printed = 0;
    let scan = grep::scan(&mut stream(b"ELF\0 line one\nline two"), &re("line", false), &mut |_, _| printed += 1).unwrap();
    assert_eq!((scan.binary, scan.matched, printed), (true, 2, 0));
    // A line longer than MAX_LINE is matched in pieces.
    let long = "x".repeat(grep::MAX_LINE + 10) + "needle\n";
    let scan = grep::scan(&mut stream(long.as_bytes()), &re("needle", false), &mut |_, _| {}).unwrap();
    assert_eq!((scan.lines, scan.matched), (2, 1));
}

#[test]
fn grep_options_and_files() {
    assert!(grep::parse("pattern").is_err(), "a path or -r is needed");
    assert!(grep::parse("-x a b").is_err());
    let options = grep::parse("-rl txt ram:docs").unwrap();
    assert!(options.recursive && options.names_only && !options.numbers);
    let mut failed = Vec::new();
    assert_eq!(grep::files(&mut disk(), &options, &mut |p, e| failed.push(format!("{}: {}", p, e))), ["ram:docs/a.txt", "ram:docs/b.md", "ram:docs/old/c.txt"]);
    let options = grep::parse("x ram:docs ram:notes.txt").unwrap();
    assert_eq!(grep::files(&mut disk(), &options, &mut |p, e| failed.push(format!("{}: {}", p, e))), ["ram:notes.txt"]);
    assert_eq!(failed, ["ram:docs: IS A DIRECTORY (USE -R)"]);
    assert_eq!(grep::parse("-r x").unwrap().paths, [""], "-r alone searches the boot disk");
}
