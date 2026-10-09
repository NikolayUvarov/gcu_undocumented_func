//! Syntax highlighting by file name (docs/tools §5, T4): each byte of a line gets a kind, and the line leaves a state
//! for the next one (a block comment, a string or a fenced block still open). Byte-based: text other than ASCII is
//! plain text, or part of a word, a string or a comment. No allocation. Host tests include this file through
//! tui/mod.rs.
use super::{Style, Theme};

/// What a byte is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Text, Keyword, Type, String, Number, Comment, Meta }

/// What a line leaves open for the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Normal,
    /// A block comment, nested this deep (Rust nests them).
    Comment(u8),
    /// A string that goes on: "..." (Rust), r#"..."# with this many #, or a triple quote of this byte (Python).
    Quote(Close),
    /// A Markdown block between ``` lines.
    Fence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Close { Double, Raw(u8), Triple(u8) }

/// How a language marks its metadata.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Meta { None, Rust, Preprocessor, Decorator, Section }

/// A language's lexical rules.
pub struct Language {
    pub name: &'static str,
    extensions: &'static [&'static str],
    line_comments: &'static [&'static str],
    /// The comment starts only at the start of a word (shells: `$#` is not one).
    comment_at_word: bool,
    block: Option<(&'static str, &'static str)>,
    nested: bool,
    quotes: &'static [u8],
    /// "..." may go on over lines.
    multiline: bool,
    triple: bool,
    raw: bool,
    /// 'x' is a character, 'a (Rust) a lifetime.
    chars: bool,
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    capitalized_types: bool,
    meta: Meta,
    /// A word or string followed by this byte is a key (TOML `=`, JSON `:`).
    key: Option<u8>,
    /// `$name` and `${...}` are variables (shells).
    variables: bool,
    markdown: bool,
}

const fn language(name: &'static str, extensions: &'static [&'static str]) -> Language {
    Language { name, extensions, line_comments: &[], comment_at_word: false, block: None, nested: false, quotes: b"\"", multiline: false, triple: false, raw: false,
               chars: false, keywords: &[], types: &[], capitalized_types: false, meta: Meta::None, key: None, variables: false, markdown: false }
}

const RUST: Language = Language {
    line_comments: &["//"], block: Some(("/*", "*/")), nested: true, multiline: true, raw: true, chars: true, capitalized_types: true, meta: Meta::Rust,
    keywords: &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
                "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "union", "unsafe", "use",
                "where", "while"],
    types: &["bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64"],
    ..language("Rust", &["rs"])
};

const C: Language = Language {
    line_comments: &["//"], block: Some(("/*", "*/")), quotes: b"\"", chars: true, meta: Meta::Preprocessor,
    keywords: &["break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for", "goto", "if", "inline", "register", "restrict", "return",
                "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while", "true", "false", "NULL"],
    types: &["char", "double", "float", "int", "long", "short", "signed", "unsigned", "void", "bool", "size_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t",
             "int8_t", "int16_t", "int32_t", "int64_t", "uintptr_t"],
    ..language("C", &["c", "h"])
};

const PYTHON: Language = Language {
    line_comments: &["#"], quotes: b"\"'", triple: true, capitalized_types: true, meta: Meta::Decorator,
    keywords: &["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
                "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "None", "True", "False"],
    types: &["int", "float", "str", "bytes", "bool", "list", "dict", "set", "tuple", "object"],
    ..language("Python", &["py"])
};

const SHELL: Language = Language {
    line_comments: &["#"], comment_at_word: true, quotes: b"\"'", variables: true,
    keywords: &["if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until", "case", "esac", "function", "return", "local", "export", "set",
                "source", "exit"],
    ..language("shell", &["sh", "bash"])
};

const MSH: Language = Language {
    line_comments: &["#"], comment_at_word: true, quotes: b"\"",
    keywords: &["let", "if", "else", "while", "for", "in", "fn", "return", "break", "continue", "try", "catch", "true", "false", "nil", "or", "requires"],
    ..language("msh", &["msh"])
};

const WIT: Language = Language {
    line_comments: &["//"], block: Some(("/*", "*/")),
    keywords: &["package", "interface", "world", "use", "type", "record", "enum", "variant", "flags", "resource", "func", "import", "export", "constructor", "static"],
    types: &["u8", "u16", "u32", "u64", "s8", "s16", "s32", "s64", "f32", "f64", "bool", "char", "string", "bytes", "list", "option", "result", "tuple", "borrow",
             "own", "endpoint", "memory"],
    ..language("WIT", &["wit"])
};

const TOML: Language = Language {
    line_comments: &["#"], quotes: b"\"'", triple: true, meta: Meta::Section, key: Some(b'='), keywords: &["true", "false"],
    ..language("TOML", &["toml"])
};

const INI: Language = Language {
    line_comments: &["#", ";"], quotes: b"\"", meta: Meta::Section, key: Some(b'='), keywords: &["true", "false", "yes", "no", "on", "off"],
    ..language("INI", &["ini", "cfg", "conf"])
};

const JSON: Language = Language { key: Some(b':'), keywords: &["true", "false", "null"], ..language("JSON", &["json"]) };

const MARKDOWN: Language = Language { markdown: true, ..language("Markdown", &["md"]) };

const LANGUAGES: [&Language; 10] = [&RUST, &C, &PYTHON, &SHELL, &MSH, &WIT, &TOML, &INI, &JSON, &MARKDOWN];

/// The language of a file by its extension (case ignored), if one is known.
pub fn for_path(path: &str) -> Option<&'static Language> {
    let name = path.rsplit(['/', ':']).next().unwrap_or(path);
    let (_, extension) = name.rsplit_once('.')?;
    LANGUAGES.iter().copied().find(|l| l.extensions.iter().any(|e| e.eq_ignore_ascii_case(extension)))
}

fn word_byte(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80 }

fn starts(text: &[u8], at: usize, with: &str) -> bool { text[at..].starts_with(with.as_bytes()) }

fn mark(kinds: &mut [Kind], from: usize, to: usize, kind: Kind) {
    let to = to.min(kinds.len());
    for k in &mut kinds[from.min(to)..to] { *k = kind; }
}

impl Language {
    /// The kinds of `text`'s bytes (one line, without its ending) into `kinds` (as long as `text`), given the state the
    /// line before left; returns the state this line leaves.
    pub fn line(&self, text: &[u8], state: State, kinds: &mut [Kind]) -> State {
        let kinds = &mut kinds[..text.len()];
        kinds.fill(Kind::Text);
        if self.markdown { return markdown(text, state, kinds); }
        let (mut at, mut state) = match state {
            State::Comment(depth) => self.comment(text, 0, 0, depth, kinds),
            State::Quote(close) => self.quote(text, 0, close, kinds),
            _ => (0, State::Normal),
        };
        let first = text.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(text.len());
        while at < text.len() && state == State::Normal {
            let b = text[at];
            let word_start = at == 0 || !word_byte(text[at - 1]);
            if self.line_comments.iter().any(|c| starts(text, at, c)) && (!self.comment_at_word || at == 0 || text[at - 1].is_ascii_whitespace()) {
                mark(kinds, at, text.len(), Kind::Comment);
                break;
            }
            if let Some((open, _)) = self.block.filter(|(open, _)| starts(text, at, open)) {
                (at, state) = self.comment(text, at, at + open.len(), 1, kinds);
                continue;
            }
            if at == first && self.meta == Meta::Preprocessor && b == b'#' {
                let end = self.line_comments.iter().filter_map(|c| find(text, at, c)).chain(self.block.and_then(|(o, _)| find(text, at, o))).min().unwrap_or(text.len());
                mark(kinds, at, end, Kind::Meta);
                at = end;
                continue;
            }
            if at == first && self.meta == Meta::Section && b == b'[' {
                let end = text[at..].iter().rposition(|&c| c == b']').map_or(text.len(), |p| at + p + 1);
                mark(kinds, at, end, Kind::Meta);
                at = end;
                continue;
            }
            if at == first && self.meta == Meta::Decorator && b == b'@' {
                let end = (at + 1..text.len()).find(|&i| !(word_byte(text[i]) || text[i] == b'.')).unwrap_or(text.len());
                mark(kinds, at, end, Kind::Meta);
                at = end;
                continue;
            }
            if self.meta == Meta::Rust && b == b'#' && (starts(text, at, "#[") || starts(text, at, "#![")) {
                let mut depth = 0usize;
                let mut end = text.len();
                for (i, &c) in text.iter().enumerate().skip(at) {
                    if c == b'[' { depth += 1; } else if c == b']' { depth -= 1; if depth == 0 { end = i + 1; break; } }
                }
                mark(kinds, at, end, Kind::Meta);
                at = end;
                continue;
            }
            if self.raw && word_start && b == b'r' {
                let hashes = text[at + 1..].iter().take_while(|&&c| c == b'#').count();
                if text.get(at + 1 + hashes) == Some(&b'"') {
                    let open = at;
                    (at, state) = self.quote(text, at + 2 + hashes, Close::Raw(hashes as u8), kinds);
                    mark(kinds, open, at, Kind::String);
                    continue;
                }
            }
            if self.triple && (starts(text, at, "\"\"\"") || starts(text, at, "'''")) {
                let open = at;
                (at, state) = self.quote(text, at + 3, Close::Triple(b), kinds);
                mark(kinds, open, at, Kind::String);
                continue;
            }
            if self.quotes.contains(&b) && !(self.chars && b == b'\'') {
                let open = at;
                // Only a language whose strings go on over lines (Rust) keeps one open.
                let end = if b == b'"' && self.multiline { let (end, left) = self.quote(text, at + 1, Close::Double, kinds); state = left; end }
                          else { single(text, at + 1, b) };
                mark(kinds, open, end, Kind::String);
                if self.key.is_some() && self.is_key(text, end) { mark(kinds, open, end, Kind::Type); }
                at = end;
                continue;
            }
            if self.chars && b == b'\'' {
                if let Some(end) = character(text, at) {
                    mark(kinds, at, end, Kind::String);
                    at = end;
                } else {
                    // A lifetime or label: 'a.
                    let end = (at + 1..text.len()).find(|&i| !word_byte(text[i])).unwrap_or(text.len());
                    mark(kinds, at, end, Kind::Type);
                    at = end.max(at + 1);
                }
                continue;
            }
            if self.variables && b == b'$' {
                let end = if text.get(at + 1) == Some(&b'{') { text[at..].iter().position(|&c| c == b'}').map_or(text.len(), |p| at + p + 1) }
                          else { (at + 1..text.len()).find(|&i| !word_byte(text[i])).unwrap_or(text.len()).max((at + 2).min(text.len())) };
                mark(kinds, at, end, Kind::Type);
                at = end;
                continue;
            }
            if word_start && (b.is_ascii_digit() || (b == b'-' && self.key == Some(b':') && text.get(at + 1).is_some_and(u8::is_ascii_digit))) {
                let mut end = at + 1;
                while end < text.len() {
                    let c = text[end];
                    let exponent = (c == b'+' || c == b'-') && matches!(text[end - 1], b'e' | b'E') && !starts(text, at, "0x");
                    if word_byte(c) || exponent || (c == b'.' && text.get(end + 1).is_some_and(u8::is_ascii_digit)) { end += 1; } else { break; }
                }
                mark(kinds, at, end, Kind::Number);
                at = end;
                continue;
            }
            if word_start && word_byte(b) {
                let end = (at..text.len()).find(|&i| !word_byte(text[i])).unwrap_or(text.len());
                let word = &text[at..end];
                let kind = if self.keywords.iter().any(|k| k.as_bytes() == word) { Kind::Keyword }
                           else if self.types.iter().any(|t| t.as_bytes() == word) || (self.capitalized_types && b.is_ascii_uppercase()) { Kind::Type }
                           else if self.meta == Meta::Rust && text.get(end) == Some(&b'!') && text.get(end + 1) != Some(&b'=') { mark(kinds, at, end + 1, Kind::Meta); at = end + 1; continue; }
                           else if self.key.is_some() && at == first && self.is_key(text, end) { Kind::Type }
                           else { Kind::Text };
                mark(kinds, at, end, kind);
                at = end;
                continue;
            }
            at += 1;
        }
        state
    }

    // A block comment that began at `begin`, scanned from `at`, `depth` deep: where it ends on this line, or the state
    // it leaves.
    fn comment(&self, text: &[u8], begin: usize, mut at: usize, mut depth: u8, kinds: &mut [Kind]) -> (usize, State) {
        let (open, close) = self.block.unwrap_or(("/*", "*/"));
        while at < text.len() {
            if starts(text, at, close) {
                at += close.len();
                depth -= 1;
                if depth == 0 { mark(kinds, begin, at, Kind::Comment); return (at, State::Normal); }
            } else if self.nested && starts(text, at, open) {
                at += open.len();
                depth = depth.saturating_add(1);
            } else {
                at += 1;
            }
        }
        mark(kinds, begin, text.len(), Kind::Comment);
        (text.len(), State::Comment(depth))
    }

    // A string from `at` (after its opening quote): where it ends on this line, or the state it leaves.
    fn quote(&self, text: &[u8], mut at: usize, close: Close, kinds: &mut [Kind]) -> (usize, State) {
        let begin = at;
        while at < text.len() {
            match close {
                Close::Double => {
                    if text[at] == b'\\' { at += 2; continue; }
                    if text[at] == b'"' { at += 1; mark(kinds, begin, at, Kind::String); return (at, State::Normal); }
                }
                Close::Raw(hashes) => {
                    if text[at] == b'"' && text[at + 1..].iter().take(hashes as usize).filter(|&&c| c == b'#').count() == hashes as usize {
                        at += 1 + hashes as usize;
                        mark(kinds, begin, at, Kind::String);
                        return (at, State::Normal);
                    }
                }
                Close::Triple(q) => {
                    if text[at] == b'\\' { at += 2; continue; }
                    if text[at..].starts_with(&[q, q, q]) { at += 3; mark(kinds, begin, at, Kind::String); return (at, State::Normal); }
                }
            }
            at += 1;
        }
        mark(kinds, begin, text.len(), Kind::String);
        (text.len(), State::Quote(close))
    }

    // Whether only spaces stand between `end` and the language's key mark.
    fn is_key(&self, text: &[u8], end: usize) -> bool {
        self.key.is_some_and(|k| text[end.min(text.len())..].iter().find(|c| !c.is_ascii_whitespace()) == Some(&k))
    }
}

fn find(text: &[u8], from: usize, what: &str) -> Option<usize> { (from..text.len()).find(|&i| starts(text, i, what)) }

// The end of a string opened by quote `q` before `at`, on this line (escapes skipped).
fn single(text: &[u8], mut at: usize, q: u8) -> usize {
    while at < text.len() {
        if text[at] == b'\\' { at += 2; continue; }
        if text[at] == q { return at + 1; }
        at += 1;
    }
    text.len()
}

// A character literal at `at` ('x', '\n', '\u{..}', one UTF-8 character): its end.
fn character(text: &[u8], at: usize) -> Option<usize> {
    let rest = &text[at + 1..];
    let len = match *rest.first()? {
        b'\\' => 1 + rest[1..].iter().position(|&c| c == b'\'')?,
        b'\'' => return None,
        b if b < 0x80 => 1,
        b if b >= 0xF0 => 4,
        b if b >= 0xE0 => 3,
        _ => 2,
    };
    (rest.get(len) == Some(&b'\'')).then_some(at + 2 + len)
}

// Markdown: headings, quotes, list marks, fenced blocks, `code` and link targets.
fn markdown(text: &[u8], state: State, kinds: &mut [Kind]) -> State {
    let first = text.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(text.len());
    let fence = text[first..].starts_with(b"```") || text[first..].starts_with(b"~~~");
    if state == State::Fence {
        mark(kinds, 0, text.len(), if fence { Kind::Meta } else { Kind::String });
        return if fence { State::Normal } else { State::Fence };
    }
    if fence { mark(kinds, 0, text.len(), Kind::Meta); return State::Fence; }
    let hashes = text[first..].iter().take_while(|&&c| c == b'#').count();
    if (1..=6).contains(&hashes) && text.get(first + hashes).is_none_or(|c| *c == b' ') { mark(kinds, 0, text.len(), Kind::Keyword); return State::Normal; }
    if text.get(first) == Some(&b'>') { mark(kinds, 0, text.len(), Kind::Comment); return State::Normal; }
    let mut at = first;
    if matches!(text.get(first), Some(b'-' | b'*' | b'+')) && text.get(first + 1) == Some(&b' ') { mark(kinds, first, first + 1, Kind::Meta); at = first + 2; }
    else {
        let digits = text[first..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && text.get(first + digits) == Some(&b'.') && text.get(first + digits + 1) == Some(&b' ') { mark(kinds, first, first + digits + 1, Kind::Meta); at = first + digits + 2; }
    }
    while at < text.len() {
        if text[at] == b'`' {
            let ticks = text[at..].iter().take_while(|&&c| c == b'`').count();
            let close = (at + ticks..text.len()).find(|&i| text[i..].starts_with(&text[at..at + ticks]));
            let end = close.map_or(text.len(), |c| c + ticks);
            mark(kinds, at, end, Kind::String);
            at = end;
        } else if text[at..].starts_with(b"](") {
            let end = text[at..].iter().position(|&c| c == b')').map_or(text.len(), |p| at + p + 1);
            mark(kinds, at + 1, end, Kind::Type);
            at = end;
        } else {
            at += 1;
        }
    }
    State::Normal
}

/// The colour of a kind in a theme: on the classic blue panels the editor's colours of Borland's IDEs, on dark ones
/// softer ones.
pub fn style(theme: &Theme, kind: Kind) -> Style {
    let classic = theme.panel.bg == super::CLASSIC.panel.bg;
    let fg = match (kind, classic) {
        (Kind::Text, _) => return theme.panel,
        (Kind::Keyword, true) => 0xFFFFFF,
        (Kind::Type, true) => 0x55FF55,
        (Kind::String, true) => 0xFFFF55,
        (Kind::Number, true) => 0xFF55FF,
        (Kind::Comment, true) => 0xAAAAAA,
        (Kind::Meta, true) => 0xFF5555,
        (Kind::Keyword, false) => 0xC792EA,
        (Kind::Type, false) => 0x82AAFF,
        (Kind::String, false) => 0xC3E88D,
        (Kind::Number, false) => 0xF78C6C,
        (Kind::Comment, false) => 0x708090,
        (Kind::Meta, false) => 0xFFCB6B,
    };
    Style::new(fg, theme.panel.bg)
}
