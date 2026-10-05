//! Name masks and simple regular expressions for `fm`, `find` and `grep`. Matching folds case for every script Rust
//! knows (Latin, Cyrillic, ...) when asked; it works on characters, not bytes. The masks are `mind::mask` (no
//! allocation, so the shell can use them too).
use alloc::vec::Vec;

pub use crate::mask::{glob, matches};

/// Why a regular expression was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternError { UnclosedClass, TrailingBackslash, NothingToRepeat, TooLong }

#[derive(Clone, Debug, PartialEq, Eq)]
enum Atom { Char(char), Any, Class { ranges: Vec<(char, char)>, negated: bool } }

#[derive(Clone, Debug, PartialEq, Eq)]
struct Piece { atom: Atom, repeat: bool }

/// A simple regular expression: literal characters, `.` any character, `[a-z]` and `[^...]` classes, `*` after an
/// atom (zero or more), `^` and `$` anchors, `\` to take the next character literally. Compiled once, matched against
/// lines with backtracking (lines are bounded by the callers).
#[derive(Clone, Debug)]
pub struct Pattern { pieces: Vec<Piece>, start: bool, end: bool, fold: bool }

const MAX_PIECES: usize = 256;

fn fold_char(c: char, fold: bool) -> char { if fold { c.to_lowercase().next().unwrap_or(c) } else { c } }

impl Pattern {
    pub fn new(text: &str, fold: bool) -> Result<Self, PatternError> {
        let chars: Vec<char> = text.chars().collect();
        let (mut i, mut end) = (0usize, chars.len());
        let start = chars.first() == Some(&'^');
        if start { i = 1; }
        let anchored_end = end > i && chars[end - 1] == '$' && !(end >= 2 && chars[end - 2] == '\\' && end - 2 >= i);
        if anchored_end { end -= 1; }
        let mut pieces: Vec<Piece> = Vec::new();
        while i < end {
            let c = chars[i];
            let atom = match c {
                '.' => { i += 1; Atom::Any }
                '*' => {
                    let last = pieces.last_mut().ok_or(PatternError::NothingToRepeat)?;
                    if last.repeat { return Err(PatternError::NothingToRepeat); }
                    last.repeat = true; i += 1; continue;
                }
                '\\' => { if i + 1 >= end { return Err(PatternError::TrailingBackslash); } i += 2; Atom::Char(fold_char(chars[i - 1], fold)) }
                '[' => {
                    i += 1;
                    let negated = i < end && chars[i] == '^';
                    if negated { i += 1; }
                    let mut ranges = Vec::new();
                    let mut first = true;
                    loop {
                        if i >= end { return Err(PatternError::UnclosedClass); }
                        if chars[i] == ']' && !first { i += 1; break; }
                        let low = fold_char(chars[i], fold);
                        if i + 2 < end && chars[i + 1] == '-' && chars[i + 2] != ']' { ranges.push((low, fold_char(chars[i + 2], fold))); i += 3; } else { ranges.push((low, low)); i += 1; }
                        first = false;
                    }
                    Atom::Class { ranges, negated }
                }
                _ => { i += 1; Atom::Char(fold_char(c, fold)) }
            };
            pieces.push(Piece { atom, repeat: false });
            if pieces.len() > MAX_PIECES { return Err(PatternError::TooLong); }
        }
        Ok(Self { pieces, start, end: anchored_end, fold })
    }

    /// The pattern occurs somewhere in `line`.
    pub fn is_match(&self, line: &str) -> bool {
        let text: Vec<char> = line.chars().map(|c| fold_char(c, self.fold)).collect();
        if self.start { return self.here(0, &text, 0); }
        (0..=text.len()).any(|at| self.here(0, &text, at))
    }

    fn one(atom: &Atom, c: char) -> bool {
        match atom {
            Atom::Char(want) => *want == c,
            Atom::Any => true,
            Atom::Class { ranges, negated } => ranges.iter().any(|&(low, high)| low <= c && c <= high) != *negated,
        }
    }

    fn here(&self, piece: usize, text: &[char], at: usize) -> bool {
        let Some(p) = self.pieces.get(piece) else { return !self.end || at == text.len() };
        if p.repeat {
            // Longest run first, then shorter ones.
            let mut run = 0;
            while at + run < text.len() && Self::one(&p.atom, text[at + run]) { run += 1; }
            (0..=run).rev().any(|n| self.here(piece + 1, text, at + n))
        } else {
            at < text.len() && Self::one(&p.atom, text[at]) && self.here(piece + 1, text, at + 1)
        }
    }
}
