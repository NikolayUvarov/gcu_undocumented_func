//! Paths from a current directory (000-APP-0066): the shell's and `console`'s `cd`, and Tab completing a path. A
//! directory is written as `fs::split` reads it: `diag`, `ram:`, `ram:notes`; the boot disk's root is the empty
//! string. No system calls: this file builds on the host for tests (tests/path_host.rs).
use alloc::string::String;
use alloc::vec::Vec;

// The volume and the parts below its root of `path`, as `fs::split` splits it.
fn split(path: &str) -> (&str, &str) {
    match path.find(':') { Some(i) if !path[..i].contains('/') => (&path[..i], &path[i + 1..]), _ => ("", path) }
}

fn join(volume: &str, parts: &[&str]) -> String {
    let mut out = String::new();
    if !volume.is_empty() { out.push_str(&volume.to_ascii_lowercase()); out.push(':'); }
    out.push_str(&parts.join("/"));
    out
}

/// `path` taken from the directory `cwd`: a volume (`ram:x`) or a leading `/` (the boot disk) starts at a root; `.`
/// is the directory, `..` the one above (none above a root). The result has no `.`, `..`, empty parts or a `/` at
/// either end.
pub fn resolve(cwd: &str, path: &str) -> String {
    let path = path.trim();
    let (volume, rest) = split(path);
    let absolute = !volume.is_empty() || path.starts_with('/');
    let (volume, mut parts): (&str, Vec<&str>) = if absolute { (volume, Vec::new()) } else {
        let (v, d) = split(cwd);
        (v, d.split('/').filter(|p| !p.is_empty() && *p != ".").collect())
    };
    for part in rest.split('/') {
        match part { "" | "." => {} ".." => { parts.pop(); } _ => parts.push(part) }
    }
    join(volume, &parts)
}

/// Whether `word` is a path to take from the current directory: not an option, a volume or the boot disk's root.
pub fn relative(word: &str) -> bool { !word.is_empty() && !word.starts_with('-') && !word.starts_with('/') && split(word).0.is_empty() }

/// The directory for `pwd`: `/` and `/diag` on the boot disk, `ram:` and `ram:notes` elsewhere.
pub fn shown(cwd: &str) -> String {
    let (volume, dir) = split(cwd);
    if volume.is_empty() { let mut s = String::from("/"); s.push_str(dir); s } else { String::from(cwd) }
}

/// The directory in the prompt: nothing at the boot disk's root (the prompt stays `MIND> `), else ` diag`, ` ram:`.
pub fn prompt(cwd: &str) -> String { if cwd.is_empty() { String::new() } else { let mut s = String::from(" "); s.push_str(cwd); s } }

/// A word being completed, split at the directory it names: `diag/q` → (`diag/`, `q`), `ram:` → (`ram:`, ``), `q` →
/// (``, `q`). The first part is kept as typed; resolved from the current directory it is the directory to list.
pub fn split_word(word: &str) -> (&str, &str) {
    match word.rfind('/') {
        Some(i) => (&word[..=i], &word[i + 1..]),
        None => match word.find(':') { Some(i) => (&word[..=i], &word[i + 1..]), None => ("", word) },
    }
}

/// What Tab does with `typed`, the start of a name, given the names it may complete to (`true`: a directory).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Nothing matches.
    None,
    /// Text to add: the rest of the one match (with `/` after a directory, a space after a file or a command), or the
    /// part every match shares.
    Add(String),
    /// The matches share nothing beyond what was typed: they are listed (a directory with `/`).
    List(Vec<String>),
}

// The length in bytes of the longest prefix `a` and `b` share, by characters, letter case aside.
fn common(a: &str, b: &str) -> usize {
    a.char_indices().zip(b.chars()).take_while(|((_, x), y)| x.to_lowercase().eq(y.to_lowercase())).last().map_or(0, |((i, x), _)| i + x.len_utf8())
}

/// Completes `typed` from `names`, letter case aside (FAT keeps names as written and finds them in any case).
pub fn complete(typed: &str, names: &[(&str, bool)]) -> Completion {
    let starts = |name: &str| name.len() >= typed.len() && common(name, typed) == typed.len();
    let mut found: Vec<(&str, bool)> = names.iter().copied().filter(|(n, _)| starts(n)).collect();
    found.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    found.dedup_by(|a, b| a.0 == b.0);
    match found.as_slice() {
        [] => Completion::None,
        [(name, dir)] => { let mut add = String::from(&name[typed.len()..]); add.push(if *dir { '/' } else { ' ' }); Completion::Add(add) }
        [first, rest @ ..] => {
            let shared = rest.iter().fold(first.0.len(), |len, (n, _)| len.min(common(&first.0[..len], n)));
            if shared > typed.len() { Completion::Add(String::from(&first.0[typed.len()..shared])) }
            else { Completion::List(found.iter().map(|(n, d)| { let mut s = String::from(*n); if *d { s.push('/'); } s }).collect()) }
        }
    }
}
