//! `|` (000-APP-0074): a command line split at its pipe, the command before it, whose output becomes the text input of
//! the program after it (`ps | grep wm`). Host-tested in tests/shell_host.rs.

/// A line split at its pipe: what writes, and the program that reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split<'a> { pub writer: &'a str, pub reader: &'a str }

/// `line` split at its first `|` outside double quotes; None without one; Err says what is wrong with it.
pub fn split(line: &str) -> Result<Option<Split<'_>>, &'static str> {
    let mut quoted = false;
    let Some(at) = line.char_indices().find_map(|(i, c)| { if c == '"' { quoted = !quoted; } (c == '|' && !quoted).then_some(i) }) else { return Ok(None) };
    let (writer, reader) = (line[..at].trim(), line[at + 1..].trim());
    if writer.is_empty() || reader.is_empty() { return Err("A COMMAND COMES ON EACH SIDE OF | (PS | GREP WM)"); }
    if reader.starts_with('|') { return Err("|| IS NOT A PIPE: ONE | BETWEEN TWO COMMANDS"); }
    if reader.contains('|') { return Err("ONE | A LINE: THE PROGRAM AFTER IT READS WHAT THE COMMAND BEFORE IT PRINTS"); }
    if writer.ends_with('&') || reader.ends_with('&') { return Err("| RUNS IN FRONT: NO & ON EITHER SIDE"); }
    Ok(Some(Split { writer, reader }))
}
