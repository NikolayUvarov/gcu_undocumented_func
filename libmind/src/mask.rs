//! Wildcard name masks: `*` any run, `?` one character, case-insensitive for every script Rust knows (Latin,
//! Cyrillic, ...), on characters, without allocating — for `fm`, `find` and the shell's `list`. This file builds on
//! the host for tests.

/// Several masks separated by `,`, `;` or spaces; any of them matching is enough. `*.*` matches names without an
/// extension too (as in DOS).
pub fn matches(masks: &str, name: &str) -> bool {
    masks.split([',', ';', ' ']).filter(|m| !m.is_empty()).any(|m| m == "*.*" || glob(m, name))
}

fn same(a: char, b: char) -> bool { a == b || a.to_lowercase().eq(b.to_lowercase()) }

/// One wildcard mask against a name, case-insensitive.
pub fn glob(mask: &str, name: &str) -> bool {
    // Byte offsets into both; after a `*` a mismatch retries from one character further in the name.
    let (mut i, mut j, mut star, mut mark) = (0usize, 0usize, None, 0usize);
    while let Some(n) = name[j..].chars().next() {
        match mask[i..].chars().next() {
            Some(m) if m == '?' || same(m, n) => { i += m.len_utf8(); j += n.len_utf8(); }
            Some('*') => { star = Some(i); i += 1; mark = j; }
            _ => match star {
                Some(s) => { i = s + 1; mark += name[mark..].chars().next().map_or(1, char::len_utf8); j = mark; }
                None => return false,
            },
        }
    }
    mask[i..].chars().all(|m| m == '*')
}
