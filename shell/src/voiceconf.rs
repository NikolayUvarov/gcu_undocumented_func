//! data/voice.conf (000-APP-0052): whether the shell turns voice control on at its start, and with which of `voice on`'s
//! words. Voice control needs the shell's own endpoint, which only the shell makes, so the shell starts it, as `voice on`
//! does, rather than `init`. No system calls: tests/shell_host.rs.
use alloc::format;
use alloc::string::String;

pub const FILE: &str = "data/voice.conf";

pub const USAGE: &str = "USAGE: VOICE ON [--wav FILE] [SECONDS] | VOICE OFF | VOICE LISTEN | VOICE ENABLE [--wav FILE] [SECONDS] | VOICE DISABLE";

/// The file `voice enable` writes: a note, then `on` and `voice on`'s words.
pub fn render(words: &str) -> String {
    format!("# voice control at the shell's start (000-APP-0052); `voice disable` removes this file\non{}{}\n", if words.is_empty() { "" } else { " " }, words)
}

/// The words of the file's first line that is not a note: after `on`, or None for another line, `off` or none.
pub fn parse(text: &str) -> Option<&str> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    if line == "on" { Some("") } else { line.strip_prefix("on ").map(str::trim).filter(|w| words_ok(w)) }
}

/// `voice on`'s words: `[--wav FILE] [SECONDS]`, a file of up to 63 bytes and seconds of 1 to 3600.
pub fn words_ok(words: &str) -> bool {
    let mut it = words.split_whitespace().peekable();
    if it.peek() == Some(&"--wav") {
        it.next();
        match it.next() { Some(file) if file.len() <= 63 && !file.starts_with('-') => {} _ => return false }
    }
    if let Some(seconds) = it.next() {
        if !matches!(seconds.parse::<u32>(), Ok(1..=3600)) { return false; }
    }
    it.next().is_none()
}
