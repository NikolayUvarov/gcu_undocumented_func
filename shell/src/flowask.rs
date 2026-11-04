//! A running program's question about a flow, put to the user (000-APP-0120, `netpolicy.wit` 1.3): its text, made safe
//! to show, and the keys that answer it. No system calls: tests/shell_host.rs.

use alloc::string::String;
use core::fmt::Write;

/// How often the shell asks the broker for questions, in ms.
pub const POLL_MS: u64 = 500;
/// After a failed ask, how long the shell leaves the broker be, in ms.
pub const QUIET_MS: u64 = 2000;
/// For this long after a question shows, a key does not answer it: one typed before the user saw it, as a command's
/// letter, must not.
pub const GUARD_MS: u64 = 500;

/// The user's answer: no; once (one connection); for the rest of the program's run; always (a line of the policy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer { No, Once, Session, Always }

/// The answer a key gives: N, O, S or A in either case; Esc and Enter the default, no. None for any other key, which
/// the question does not take.
pub fn answer(ch: Option<char>, escape: bool, enter: bool) -> Option<Answer> {
    if escape || enter { return Some(Answer::No); }
    match ch.map(|c| c.to_ascii_lowercase()) {
        Some('n') => Some(Answer::No),
        Some('o') => Some(Answer::Once),
        Some('s') => Some(Answer::Session),
        Some('a') => Some(Answer::Always),
        _ => None,
    }
}

/// The answer's words, as the shell says what it gave.
pub fn word(answer: Answer) -> &'static str {
    match answer { Answer::No => "NO", Answer::Once => "ONCE", Answer::Session => "FOR THIS RUN", Answer::Always => "ALWAYS (A LINE OF THE POLICY)" }
}

/// `text` as one line of at most `max` characters: a control character, an escape sequence's start among them, is
/// `?`, so the words a program gives cannot move the cursor or draw a question of their own.
pub fn shown(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, c) in text.chars().enumerate() {
        if i == max { out.push('…'); break; }
        out.push(if c.is_control() { '?' } else { c });
    }
    out
}

/// The question's lines: who asks and for what, its own reason marked as its own, and the keys.
pub fn question(program: &str, pid: u64, host: &str, port: u16, why: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "NETWORK: {} (PID {}) ASKS TO REACH {}:{}", shown(program, 16), pid, shown(host, 253), port);
    if !why.is_empty() { let _ = writeln!(out, "  ITS REASON, IN ITS OWN WORDS: \"{}\"", shown(why, 80)); }
    let _ = write!(out, "  N: NO (THE DEFAULT)  O: ONCE  S: FOR THIS RUN  A: ALWAYS? ");
    out
}
