//! A console program's secret line (000-APP-0126, `shell.wit` 1.12): the shell's prompt for it, and the line typed with
//! no echo, wiped once given back or refused. No system calls: tests/shell_host.rs.

use crate::flowask::shown;
use alloc::format;
use alloc::string::String;

/// The longest line, in bytes (`secret-line`'s answer).
pub const MAX: usize = 256;

/// The shell's own line before the secret: who asks, and its words as its own, made safe to show.
pub fn prompt(program: &str, pid: u64, words: &str) -> String {
    format!("SECRET FOR {} (PID {}), TYPED WITH NO ECHO (ENTER ENDS IT, ESC REFUSES IT): {} ", shown(program, 16), pid, shown(words, 80))
}

/// What a key did to the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step { Going, Done, Refused }

/// The bytes typed so far. Never shown; wiped by `wipe` and when dropped.
pub struct Line { bytes: [u8; MAX], len: usize }

impl Default for Line { fn default() -> Self { Self::new() } }

impl Line {
    pub const fn new() -> Self { Self { bytes: [0; MAX], len: 0 } }

    /// A key: a printable character (none past `MAX` bytes), Backspace, Enter (done) or Esc and Ctrl+C (refused); any
    /// other key does nothing.
    pub fn key(&mut self, ch: Option<char>, escape: bool, enter: bool, backspace: bool) -> Step {
        if escape || ch == Some('\u{3}') { return Step::Refused; }
        if enter { return Step::Done; }
        if backspace {
            while self.len > 0 { self.len -= 1; let b = self.bytes[self.len]; self.bytes[self.len] = 0; if b & 0xC0 != 0x80 { break; } }
            return Step::Going;
        }
        if let Some(c) = ch.filter(|c| !c.is_control()) {
            let mut utf8 = [0u8; 4];
            let encoded = c.encode_utf8(&mut utf8).as_bytes();
            if self.len + encoded.len() <= MAX { self.bytes[self.len..self.len + encoded.len()].copy_from_slice(encoded); self.len += encoded.len(); }
        }
        Step::Going
    }

    pub fn bytes(&self) -> &[u8] { &self.bytes[..self.len] }

    /// `from`'s bytes in place of these, and `from` wiped: a line kept for a later step (308-APP-0130).
    pub fn take(&mut self, from: &mut Line) {
        self.wipe();
        self.bytes[..from.len].copy_from_slice(&from.bytes[..from.len]);
        self.len = from.len;
        from.wipe();
    }

    /// Every byte zeroed, by writes the compiler keeps.
    pub fn wipe(&mut self) {
        for b in self.bytes.iter_mut() { unsafe { core::ptr::write_volatile(b, 0) } }
        self.len = 0;
    }
}

impl Drop for Line { fn drop(&mut self) { self.wipe(); } }
