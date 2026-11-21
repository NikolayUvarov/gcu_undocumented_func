//! A strict JSON reader and a writer for `hub-api 1` (docs/hub/README.md, 10.3). The reader takes one encoding of each
//! value and refuses the rest rather than repair it:
//! - nesting deeper than DEPTH, a body longer than BODY_MAX;
//! - a key twice in one object;
//! - a number that is not an integer within i64, or has a sign on a zero, leading zeros, a fraction or an exponent;
//! - a control character in a string, a lone surrogate, an escape JSON does not have;
//! - anything after the value but white space.
//!
//! On MIND Core a program gets this tree from `parse` instead (MC-11.11) and checks it with the same `wire` functions.

use alloc::string::String;
use alloc::vec::Vec;

pub const DEPTH: usize = 8;
/// The longest body the API takes in one request or answer, attachments' bytes aside.
pub const BODY_MAX: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Value>),
    /// The keys in the order they came, each once.
    Object(Vec<(String, Value)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLong,
    TooDeep,
    /// The byte offset where the text stopped being JSON of this form.
    Syntax(usize),
    Duplicate(usize),
    Number(usize),
    Trailing(usize),
}

pub fn parse(text: &str) -> Result<Value, Error> {
    if text.len() > BODY_MAX { return Err(Error::TooLong); }
    let mut r = Reader { b: text.as_bytes(), at: 0 };
    r.space();
    let value = r.value(0)?;
    r.space();
    if r.at != r.b.len() { return Err(Error::Trailing(r.at)); }
    Ok(value)
}

struct Reader<'a> { b: &'a [u8], at: usize }

impl Reader<'_> {
    fn space(&mut self) { while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.b.get(self.at) { self.at += 1; } }

    fn eat(&mut self, c: u8) -> Result<(), Error> {
        if self.b.get(self.at) == Some(&c) { self.at += 1; Ok(()) } else { Err(Error::Syntax(self.at)) }
    }

    fn word(&mut self, w: &[u8], v: Value) -> Result<Value, Error> {
        if self.b[self.at..].starts_with(w) { self.at += w.len(); Ok(v) } else { Err(Error::Syntax(self.at)) }
    }

    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        match self.b.get(self.at) {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(Value::Str),
            Some(b't') => self.word(b"true", Value::Bool(true)),
            Some(b'f') => self.word(b"false", Value::Bool(false)),
            Some(b'n') => self.word(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(Error::Syntax(self.at)),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > DEPTH { return Err(Error::TooDeep); }
        self.eat(b'{')?;
        let mut fields: Vec<(String, Value)> = Vec::new();
        self.space();
        if self.b.get(self.at) == Some(&b'}') { self.at += 1; return Ok(Value::Object(fields)); }
        loop {
            self.space();
            let start = self.at;
            if self.b.get(self.at) != Some(&b'"') { return Err(Error::Syntax(self.at)); }
            let key = self.string()?;
            if fields.iter().any(|(k, _)| *k == key) { return Err(Error::Duplicate(start)); }
            self.space();
            self.eat(b':')?;
            self.space();
            let value = self.value(depth)?;
            fields.push((key, value));
            self.space();
            match self.b.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => { self.at += 1; return Ok(Value::Object(fields)); }
                _ => return Err(Error::Syntax(self.at)),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > DEPTH { return Err(Error::TooDeep); }
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.space();
        if self.b.get(self.at) == Some(&b']') { self.at += 1; return Ok(Value::Array(items)); }
        loop {
            self.space();
            items.push(self.value(depth)?);
            self.space();
            match self.b.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => { self.at += 1; return Ok(Value::Array(items)); }
                _ => return Err(Error::Syntax(self.at)),
            }
        }
    }

    fn number(&mut self) -> Result<Value, Error> {
        let start = self.at;
        let negative = self.b.get(self.at) == Some(&b'-');
        if negative { self.at += 1; }
        let digits = self.at;
        while let Some(b'0'..=b'9') = self.b.get(self.at) { self.at += 1; }
        let text = &self.b[digits..self.at];
        if text.is_empty() || (text.len() > 1 && text[0] == b'0') || (negative && text == b"0") { return Err(Error::Number(start)); }
        if let Some(b'.' | b'e' | b'E') = self.b.get(self.at) { return Err(Error::Number(start)); }
        let mut value: i64 = 0;
        for d in text {
            let digit = (d - b'0') as i64;
            value = value.checked_mul(10).and_then(|v| if negative { v.checked_sub(digit) } else { v.checked_add(digit) }).ok_or(Error::Number(start))?;
        }
        Ok(Value::Int(value))
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let start = self.at;
        let digits = self.b.get(self.at..self.at + 4).ok_or(Error::Syntax(start))?;
        let mut v = 0u32;
        for d in digits {
            let x = match d { b'0'..=b'9' => d - b'0', b'a'..=b'f' => d - b'a' + 10, b'A'..=b'F' => d - b'A' + 10, _ => return Err(Error::Syntax(start)) };
            v = v << 4 | x as u32;
        }
        self.at += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, Error> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.at;
            // The text is UTF-8 already (`parse` takes a &str): copy a run of plain bytes at once.
            while let Some(&c) = self.b.get(self.at) {
                if c == b'"' || c == b'\\' || c < 0x20 { break; }
                self.at += 1;
            }
            out.push_str(core::str::from_utf8(&self.b[start..self.at]).map_err(|_| Error::Syntax(start))?);
            match self.b.get(self.at) {
                Some(b'"') => { self.at += 1; return Ok(out); }
                Some(b'\\') => {
                    self.at += 1;
                    let c = *self.b.get(self.at).ok_or(Error::Syntax(self.at))?;
                    self.at += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let at = self.at;
                            let high = self.hex4()?;
                            let code = match high {
                                0xD800..=0xDBFF => {
                                    if !self.b[self.at..].starts_with(b"\\u") { return Err(Error::Syntax(at)); }
                                    self.at += 2;
                                    let low = self.hex4()?;
                                    if !(0xDC00..=0xDFFF).contains(&low) { return Err(Error::Syntax(at)); }
                                    0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
                                }
                                0xDC00..=0xDFFF => return Err(Error::Syntax(at)),
                                c => c,
                            };
                            out.push(char::from_u32(code).ok_or(Error::Syntax(at))?);
                        }
                        _ => return Err(Error::Syntax(self.at - 1)),
                    }
                }
                _ => return Err(Error::Syntax(self.at)),
            }
        }
    }
}

impl Value {
    /// An object's field, if this is an object and has it.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self { Value::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> { match self { Value::Str(s) => Some(s), _ => None } }
    pub fn as_int(&self) -> Option<i64> { match self { Value::Int(i) => Some(*i), _ => None } }
    pub fn as_bool(&self) -> Option<bool> { match self { Value::Bool(b) => Some(*b), _ => None } }
    pub fn as_array(&self) -> Option<&[Value]> { match self { Value::Array(a) => Some(a), _ => None } }
    /// The keys of an object, if it is one.
    pub fn keys(&self) -> Option<impl Iterator<Item = &str>> {
        match self { Value::Object(fields) => Some(fields.iter().map(|(k, _)| k.as_str())), _ => None }
    }
}

/// A value as MIND Core's `parse` service gives it (`parse.wit`'s `json`): flat, in document order, each with its kind,
/// the index of the object or array it is in (None for the outermost), its member's key and its text (a string's text,
/// a number's lexeme).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatKind { Object, Array, String, Number, True, False, Null }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flat<'a> { pub kind: FlatKind, pub parent: Option<u16>, pub key: &'a str, pub text: &'a str }

/// The tree of flat values, held to this reader's rules: one outermost value, members under an object or array that
/// came before them, no key twice in one object, integers only within i64 and in their one form, no deeper than DEPTH.
/// The error's offset is the value's index.
pub fn tree(values: &[Flat]) -> Result<Value, Error> {
    if values.is_empty() || values[0].parent.is_some() { return Err(Error::Syntax(0)); }
    let mut children: Vec<Vec<usize>> = (0..values.len()).map(|_| Vec::new()).collect();
    for (i, v) in values.iter().enumerate().skip(1) {
        let p = v.parent.ok_or(Error::Trailing(i))? as usize;
        if p >= i || !matches!(values[p].kind, FlatKind::Object | FlatKind::Array) { return Err(Error::Syntax(i)); }
        children[p].push(i);
    }
    build(values, &children, 0, 0)
}

fn build(values: &[Flat], children: &[Vec<usize>], i: usize, depth: usize) -> Result<Value, Error> {
    let v = &values[i];
    Ok(match v.kind {
        FlatKind::Object => {
            if depth + 1 > DEPTH { return Err(Error::TooDeep); }
            let mut fields: Vec<(String, Value)> = Vec::new();
            for &c in &children[i] {
                if fields.iter().any(|(k, _)| k == values[c].key) { return Err(Error::Duplicate(c)); }
                fields.push((String::from(values[c].key), build(values, children, c, depth + 1)?));
            }
            Value::Object(fields)
        }
        FlatKind::Array => {
            if depth + 1 > DEPTH { return Err(Error::TooDeep); }
            let mut items = Vec::new();
            for &c in &children[i] { items.push(build(values, children, c, depth + 1)?); }
            Value::Array(items)
        }
        FlatKind::String => Value::Str(String::from(v.text)),
        // The lexeme read by this reader's own rule, so a number has one form whoever parsed it.
        FlatKind::Number => match parse(v.text) { Ok(Value::Int(n)) => Value::Int(n), _ => return Err(Error::Number(i)) },
        FlatKind::True => Value::Bool(true),
        FlatKind::False => Value::Bool(false),
        FlatKind::Null => Value::Null,
    })
}

/// Writes JSON with its keys in the order given and no white space: the API's one encoding.
#[derive(Default)]
pub struct Writer { out: String, first: Vec<bool> }

impl Writer {
    pub fn new() -> Writer { Writer::default() }

    fn comma(&mut self) {
        if let Some(first) = self.first.last_mut() {
            if !*first { self.out.push(','); }
            *first = false;
        }
    }

    fn quoted(&mut self, s: &str) {
        self.out.push('"');
        for c in s.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{2028}' || c == '\u{2029}' => {
                    let _ = core::fmt::Write::write_fmt(&mut self.out, format_args!("\\u{:04x}", c as u32));
                }
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }

    pub fn begin_object(&mut self) -> &mut Self { self.comma(); self.out.push('{'); self.first.push(true); self }
    pub fn begin_array(&mut self) -> &mut Self { self.comma(); self.out.push('['); self.first.push(true); self }
    pub fn end_object(&mut self) -> &mut Self { self.first.pop(); self.out.push('}'); self }
    pub fn end_array(&mut self) -> &mut Self { self.first.pop(); self.out.push(']'); self }

    /// A key inside an object; its value follows.
    pub fn key(&mut self, k: &str) -> &mut Self {
        self.comma();
        self.quoted(k);
        self.out.push(':');
        // The value that follows writes no comma; writing it marks the object as no longer at its first key.
        if let Some(first) = self.first.last_mut() { *first = true; }
        self
    }

    pub fn string(&mut self, s: &str) -> &mut Self { self.comma(); self.quoted(s); self }
    pub fn int(&mut self, v: i64) -> &mut Self { self.comma(); let _ = core::fmt::Write::write_fmt(&mut self.out, format_args!("{}", v)); self }
    pub fn boolean(&mut self, v: bool) -> &mut Self { self.comma(); self.out.push_str(if v { "true" } else { "false" }); self }
    pub fn null(&mut self) -> &mut Self { self.comma(); self.out.push_str("null"); self }

    pub fn field_str(&mut self, k: &str, v: &str) -> &mut Self { self.key(k).string(v) }
    pub fn field_int(&mut self, k: &str, v: i64) -> &mut Self { self.key(k).int(v) }
    pub fn field_bool(&mut self, k: &str, v: bool) -> &mut Self { self.key(k).boolean(v) }

    pub fn finish(self) -> String { self.out }
}
