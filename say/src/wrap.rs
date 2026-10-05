//! say's text on its screen (issue u010): the lines of the text cut into rows of at most `columns` characters, at
//! a space where there is one. Characters, not bytes: Cyrillic is two bytes a letter. No system calls:
//! tests/say_host.rs.

/// The first row of `line` (no newline in it): at most `columns` characters, cut at the last space that fits when
/// there is one; and the rest, without the spaces it starts with.
pub fn cut(line: &str, columns: usize) -> (&str, &str) {
    let Some((end, next)) = line.char_indices().nth(columns.max(1)) else { return (line, "") };
    let at = if next == ' ' { end } else { line[..end].rfind(' ').filter(|&space| !line[..space].trim().is_empty()).unwrap_or(end) };
    (line[..at].trim_end(), line[at..].trim_start())
}

/// The rows of `text` on a screen `columns` wide: each of its lines cut by `cut`; an empty line stays an empty row.
pub fn rows(text: &str, columns: usize) -> impl Iterator<Item = &str> {
    text.lines().flat_map(move |line| {
        let mut rest = Some(line.trim_end());
        core::iter::from_fn(move || {
            let (row, next) = cut(rest?, columns);
            rest = (!next.is_empty()).then_some(next);
            Some(row)
        })
    })
}
