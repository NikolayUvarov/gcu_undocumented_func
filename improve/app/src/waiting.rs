//! The requests that wait for the user on MIND Hub, as `improve` last saw them (100003-APP-0153): `data/improve/waiting`,
//! which `improve` writes and the shell and `wm` read (both include this file). A line of request numbers apart by
//! spaces; `#` starts a comment. A request waits for its user while a question or a proposal of the agents is open, or
//! while its check waits.

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

/// The numbers in `text`, ascending, each once; None when a word is not a request's number.
pub fn read(text: &str) -> Option<Vec<u64>> {
    let mut ids = Vec::new();
    for line in text.lines() {
        for word in line.split('#').next().unwrap_or("").split_whitespace() {
            let id = word.parse::<u64>().ok().filter(|&n| n > 0)?;
            if ids.len() < MAX && !ids.contains(&id) { ids.push(id); }
        }
    }
    ids.sort_unstable();
    Some(ids)
}

/// The file's text for `ids`, which `read` gives back.
pub fn text(ids: &[u64]) -> String {
    let mut out = String::from("# The requests that wait for you on MIND Hub, as improve last saw them: improve list\n");
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
