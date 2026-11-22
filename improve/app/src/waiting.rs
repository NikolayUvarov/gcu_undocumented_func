//! The requests that wait for the user on MIND Hub, as `improve` last saw them (100003-APP-0153): `data/improve/waiting`,
//! which `improve` writes and the shell and `wm` read (both include this file). A comment line; `change N`, improve's
//! count of its writes, so that a reader that missed one, a set that went and came back between two of its looks, sees
//! that it changed; then the request numbers apart by spaces. A request waits for its user while a question or a
//! proposal of the agents is open, or while its check waits.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The file, in the user's files.
pub const FILE: &str = "data/improve/waiting";
/// Its directory, made when it is first written.
pub const DIR: &str = "data/improve";
/// The most numbers taken from it, and the most bytes read of it.
pub const MAX: usize = 32;
pub const BYTES: usize = 512;

/// What the file says: how many times improve wrote it, and the requests that wait.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Waiting { pub change: u64, pub ids: Vec<u64> }

/// The file read: its `change` (0 when it names none) and its numbers, ascending, each once; None when a word is not a
/// request's number or `change` is not one count.
pub fn read(text: &str) -> Option<Waiting> {
    let mut w = Waiting::default();
    for line in text.lines() {
        let words: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        if let ["change", n] = words.as_slice() { w.change = n.parse().ok()?; continue; }
        for word in words {
            let id = word.parse::<u64>().ok().filter(|&n| n > 0)?;
            if w.ids.len() < MAX && !w.ids.contains(&id) { w.ids.push(id); }
        }
    }
    w.ids.sort_unstable();
    Some(w)
}

/// The file's text for `ids` at `change`, which `read` gives back.
pub fn text(change: u64, ids: &[u64]) -> String {
    let mut out = format!("# The requests that wait for you on MIND Hub, as improve last saw them: improve list\nchange {change}\n");
    let words: Vec<String> = ids.iter().take(MAX).map(|id| format!("{id}")).collect();
    out.push_str(&words.join(" "));
    out.push('\n');
    out
}

/// The shell's line above its prompt; None when nothing waits.
pub fn notice(ids: &[u64]) -> Option<String> {
    match ids {
        [] => None,
        [one] => Some(format!("IMPROVE: REQUEST #{one} WAITS FOR YOU (improve show {one})")),
        _ => {
            let shown: Vec<String> = ids.iter().take(8).map(|id| format!("#{id}")).collect();
            Some(format!("IMPROVE: {} REQUESTS WAIT FOR YOU: {}{} (improve list)", ids.len(), shown.join(" "), if ids.len() > 8 { " …" } else { "" }))
        }
    }
}

/// `wm`'s mark in its top bar, ` HUB 2 `; None when nothing waits.
pub fn mark(ids: &[u64]) -> Option<String> { (!ids.is_empty()).then(|| format!(" HUB {} ", ids.len())) }

/// What a click on `wm`'s mark says on its status line.
pub fn said(ids: &[u64]) -> String {
    let shown: Vec<String> = ids.iter().take(8).map(|id| format!("#{id}")).collect();
    format!("MIND Hub: {} for you ({}{}): improve list in the shell", if ids.len() == 1 { String::from("a request waits") } else { format!("{} requests wait", ids.len()) },
            shown.join(" "), if ids.len() > 8 { " …" } else { "" })
}
