//! `>` and `>>` (000-APP-0074): a command line split at its redirection, and its output sorted line by line into the
//! file, or onto the screen for a line that starts `ERROR: `. Host-tested in tests/shell_host.rs.
use alloc::vec::Vec;

/// A line split at its redirection: the command, whether it appends (`>>`), and the file as typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split<'a> { pub command: &'a str, pub append: bool, pub file: &'a str }

/// `line` split at its first `>` outside double quotes; None without one; Err says what is wrong with it.
pub fn split(line: &str) -> Result<Option<Split<'_>>, &'static str> {
    let mut quoted = false;
    let Some(at) = line.char_indices().find_map(|(i, c)| { if c == '"' { quoted = !quoted; } (c == '>' && !quoted).then_some(i) }) else { return Ok(None) };
    let command = line[..at].trim();
    let append = line[at + 1..].starts_with('>');
    let file = line[at + if append { 2 } else { 1 }..].trim();
    if command.is_empty() { return Err("A COMMAND COMES BEFORE > (COMMAND > FILE)"); }
    if file.is_empty() { return Err("A FILE COMES AFTER > (COMMAND > FILE)"); }
    if file.contains(['>', '<', '|', '"']) || file.split_whitespace().count() > 1 { return Err("ONE FILE AFTER > OR >>, AT THE END OF THE LINE"); }
    if command.ends_with('&') { return Err("> TAKES A COMMAND IN FRONT, NOT ONE STARTED WITH &"); }
    Ok(Some(Split { command, append, file }))
}

/// Output sorted line by line: a line that starts `ERROR: ` is shown, the rest goes into the file. A line longer than
/// `LONG` without its end goes into the file as it is.
#[derive(Default)]
pub struct Lines { line: Vec<u8> }

pub const LONG: usize = 4096;

impl Lines {
    /// Sorts `bytes`: complete lines into `file` or `shown`; a line not ended yet waits for the rest.
    pub fn feed(&mut self, bytes: &[u8], file: &mut Vec<u8>, shown: &mut Vec<u8>) {
        for &byte in bytes {
            self.line.push(byte);
            if byte == b'\n' || self.line.len() >= LONG { self.end(file, shown); }
        }
    }
    /// The line not ended, where it belongs.
    pub fn finish(&mut self, file: &mut Vec<u8>, shown: &mut Vec<u8>) { if !self.line.is_empty() { self.end(file, shown); } }

    fn end(&mut self, file: &mut Vec<u8>, shown: &mut Vec<u8>) {
        let to = if self.line.starts_with(b"ERROR: ") { shown } else { file };
        to.extend_from_slice(&self.line);
        self.line.clear();
    }
}
