//! The text side of the shell's `nn load | unload | run | bench` (262-APP-0112, docs/nn/runtime.md section 7): a model
//! ID in hex, a file of rows read as numbers, how many rows one job carries, and output rows as text. No system calls:
//! tests/shell_host.rs.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

/// What one `submit` or `wait` carries at most (`nn.wit`'s `bytes<61440>`).
pub const CALL_BYTES: usize = 61_440;

/// The 64 hex digits of a model ID.
pub fn id(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.is_ascii() { return None; }
    let mut id = [0u8; 32];
    for (i, byte) in id.iter_mut().enumerate() { *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?; }
    Some(id)
}

/// The ID as the shell shows it: its first 8 bytes and an ellipsis.
pub fn short(id: &[u8]) -> String {
    let mut text = String::new();
    for byte in id.iter().take(8) { let _ = write!(text, "{:02x}", byte); }
    text.push('…');
    text
}

/// The file `nn run <id> <file>` names, which the shell reads from the current directory first.
pub fn run_file(args: &str) -> Option<&str> {
    let mut words = args.split_whitespace();
    (words.next() == Some("run")).then(|| { words.next(); words.next() }).flatten()
}

/// A file's rows, `width` finite numbers each, apart by spaces, tabs or commas; an empty line and one that starts `#`
/// are skipped. The values in order, or the first line that is not a row, and why.
pub fn rows(text: &str, width: usize) -> Result<Vec<f64>, String> {
    let mut values = Vec::new();
    for (at, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let before = values.len();
        for word in line.split(|c: char| c == ',' || c.is_whitespace()).filter(|w| !w.is_empty()) {
            match word.parse::<f64>() {
                Ok(value) if value.is_finite() => values.push(value),
                Ok(_) => return Err(alloc::format!("LINE {}: {} IS NOT A FINITE NUMBER", at + 1, shown(word))),
                Err(_) => return Err(alloc::format!("LINE {}: {} IS NOT A NUMBER", at + 1, shown(word))),
            }
        }
        let got = values.len() - before;
        if got != width { return Err(alloc::format!("LINE {}: {} NUMBERS, THE MODEL TAKES {}", at + 1, got, width)); }
    }
    Ok(values)
}

/// The rows one job carries: the model's limit, and what one call's bytes hold of its input and of its output rows; 0
/// when not one row fits.
pub fn per_job(rows_max: u32, inputs: u32, outputs: u32) -> usize {
    let row = |width: u32| (width as usize).saturating_mul(8).max(1);
    (rows_max as usize).min(CALL_BYTES / row(inputs)).min(CALL_BYTES / row(outputs))
}

/// Values as little-endian f64, as `submit` takes them.
pub fn encode(values: &[f64]) -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() }

/// One output row as text: its values apart by a space, each as the shortest decimal that reads back as it.
pub fn row_text(bytes: &[u8]) -> String {
    let mut text = String::new();
    for (i, value) in bytes.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap_or([0; 8]))).enumerate() {
        if i > 0 { text.push(' '); }
        let _ = write!(text, "{}", value);
    }
    text
}

/// Rows a second over `ms` milliseconds, in whole rows.
pub fn rate(rows: u64, ms: u64) -> u64 { rows.saturating_mul(1000) / ms.max(1) }

// A word of the file as the error shows it: 16 characters at most, control characters as `?`.
fn shown(word: &str) -> String {
    let mut text: String = word.chars().take(16).map(|c| if c.is_control() { '?' } else { c }).collect();
    if word.chars().count() > 16 { text.push('…'); }
    text
}
