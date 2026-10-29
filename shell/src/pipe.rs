//! `|` (000-APP-0074): a command line split at its pipes, the command before each, whose output becomes the text input
//! of the program after it (`ps | grep wm`, `dmesg | grep SHELL | grep -c START`). Host-tested in tests/shell_host.rs.

/// A line split at its first pipe: what writes, and the rest, a program that reads or another pipe of programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split<'a> { pub writer: &'a str, pub reader: &'a str }

/// The first position of `sign` outside double quotes.
pub fn find(line: &str, sign: char) -> Option<usize> {
    let mut quoted = false;
    line.char_indices().find_map(|(i, c)| { if c == '"' { quoted = !quoted; } (c == sign && !quoted).then_some(i) })
}

/// `line` split at its first `|` outside double quotes, the rest of it checked too; None without one; Err says what is
/// wrong with it.
pub fn split(line: &str) -> Result<Option<Split<'_>>, &'static str> {
    let Some(at) = find(line, '|') else { return Ok(None) };
    let (writer, reader) = (line[..at].trim(), line[at + 1..].trim());
    if writer.is_empty() || reader.is_empty() { return Err("A COMMAND COMES ON EACH SIDE OF | (PS | GREP WM)"); }
    if reader.starts_with('|') { return Err("|| IS NOT A PIPE: ONE | BETWEEN TWO COMMANDS"); }
    if writer.ends_with('&') || reader.ends_with('&') { return Err("| RUNS IN FRONT: NO & ON EITHER SIDE"); }
    if find(reader, '<').is_some() { return Err("< GIVES THE FIRST COMMAND ITS INPUT: GREP A < FILE | GREP B"); }
    split(reader)?;
    Ok(Some(Split { writer, reader }))
}

/// The programs after the pipes of `split`, in order: each reads what the one before it prints.
pub fn readers<'a>(split: Split<'a>) -> impl Iterator<Item = &'a str> {
    let mut rest = Some(split.reader);
    core::iter::from_fn(move || {
        let line = rest?;
        match self::split(line) { Ok(Some(next)) => { rest = Some(next.reader); Some(next.writer) } _ => { rest = None; Some(line) } }
    })
}
