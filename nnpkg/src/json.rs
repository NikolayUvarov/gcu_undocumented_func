//! A strict JSON reader for nnd's packages (262-APP-0112, docs/nn/runtime.md section 6): RFC 8259 as `serde_json`
//! reads it, a duplicate key refused, the nesting and an object's keys bounded; numbers kept as their text.
use alloc::string::String;
use alloc::vec::Vec;

/// The deepest nesting read; a package's records nest 7 deep at most.
pub const DEPTH: usize = 32;
/// The most keys of one object; a valid package's largest object is the manifest's 64 objects.
pub const KEYS: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Null,
    Bool(bool),
    /// The number's literal, checked against the grammar.
    Number(&'a str),
    String(String),
    Array(Vec<Value<'a>>),
    /// Members in the file's order, each key once.
    Object(Vec<(String, Value<'a>)>),
}

/// The value `text` holds, alone but for white space around it.
pub fn parse(text: &[u8]) -> Result<Value<'_>, &'static str> {
    let text = core::str::from_utf8(text).map_err(|_| "NOT UTF-8")?;
    let mut reader = Reader { text, at: 0 };
    let value = reader.value(0)?;
    reader.space();
    if reader.at != text.len() { return Err("TEXT AFTER THE VALUE"); }
    Ok(value)
}

struct Reader<'a> { text: &'a str, at: usize }

impl<'a> Reader<'a> {
    fn bytes(&self) -> &'a [u8] { self.text.as_bytes() }
    fn peek(&self) -> Option<u8> { self.bytes().get(self.at).copied() }
    fn space(&mut self) { while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) { self.at += 1; } }
    fn literal(&mut self, word: &str) -> Result<(), &'static str> {
        if self.text[self.at..].starts_with(word) { self.at += word.len(); Ok(()) } else { Err("NOT A JSON VALUE") }
    }

    fn value(&mut self, depth: usize) -> Result<Value<'a>, &'static str> {
        self.space();
        match self.peek().ok_or("THE TEXT ENDS EARLY")? {
            b'{' => self.object(depth + 1),
            b'[' => self.array(depth + 1),
            b'"' => self.string().map(Value::String),
            b't' => self.literal("true").map(|_| Value::Bool(true)),
            b'f' => self.literal("false").map(|_| Value::Bool(false)),
            b'n' => self.literal("null").map(|_| Value::Null),
            b'-' | b'0'..=b'9' => self.number().map(Value::Number),
            _ => Err("NOT A JSON VALUE"),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value<'a>, &'static str> {
        if depth > DEPTH { return Err("NESTED TOO DEEP"); }
        self.at += 1;
        let mut members: Vec<(String, Value<'a>)> = Vec::new();
        self.space();
        if self.peek() == Some(b'}') { self.at += 1; return Ok(Value::Object(members)); }
        loop {
            self.space();
            if self.peek() != Some(b'"') { return Err("AN OBJECT'S KEY IS NOT A STRING"); }
            let key = self.string()?;
            if members.iter().any(|(k, _)| *k == key) { return Err("A DUPLICATE KEY"); }
            if members.len() == KEYS { return Err("AN OBJECT WITH TOO MANY KEYS"); }
            self.space();
            if self.peek() != Some(b':') { return Err("NO ':' AFTER A KEY"); }
            self.at += 1;
            let value = self.value(depth)?;
            members.push((key, value));
            self.space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => { self.at += 1; return Ok(Value::Object(members)); }
                _ => return Err("NO ',' OR '}' IN AN OBJECT"),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value<'a>, &'static str> {
        if depth > DEPTH { return Err("NESTED TOO DEEP"); }
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.peek() == Some(b']') { self.at += 1; return Ok(Value::Array(items)); }
        loop {
            items.push(self.value(depth)?);
            self.space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => { self.at += 1; return Ok(Value::Array(items)); }
                _ => return Err("NO ',' OR ']' IN AN ARRAY"),
            }
        }
    }

    // -?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?
    fn number(&mut self) -> Result<&'a str, &'static str> {
        let start = self.at;
        let digits = |r: &mut Self| { let s = r.at; while matches!(r.peek(), Some(b'0'..=b'9')) { r.at += 1; } r.at - s };
        if self.peek() == Some(b'-') { self.at += 1; }
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => { digits(self); }
            _ => return Err("A MALFORMED NUMBER"),
        }
        if self.peek() == Some(b'.') { self.at += 1; if digits(self) == 0 { return Err("A MALFORMED NUMBER"); } }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) { self.at += 1; }
            if digits(self) == 0 { return Err("A MALFORMED NUMBER"); }
        }
        Ok(&self.text[start..self.at])
    }

    fn hex4(&mut self) -> Result<u32, &'static str> {
        let digits = self.bytes().get(self.at..self.at + 4).ok_or("A SHORT \\u ESCAPE")?;
        let mut value = 0;
        for &d in digits { value = value * 16 + (d as char).to_digit(16).ok_or("A MALFORMED \\u ESCAPE")?; }
        self.at += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, &'static str> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while let Some(b) = self.peek() { if b == b'"' || b == b'\\' || b < 0x20 { break; } self.at += 1; }
            out.push_str(&self.text[start..self.at]);
            match self.peek().ok_or("A STRING NOT CLOSED")? {
                b'"' => { self.at += 1; return Ok(out); }
                b'\\' => {
                    self.at += 1;
                    let escape = self.peek().ok_or("A STRING NOT CLOSED")?;
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let code = match first {
                                0xD800..=0xDBFF => {
                                    if !self.text[self.at..].starts_with("\\u") { return Err("A LONE SURROGATE"); }
                                    self.at += 2;
                                    let second = self.hex4()?;
                                    if !(0xDC00..=0xDFFF).contains(&second) { return Err("A LONE SURROGATE"); }
                                    0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                                }
                                0xDC00..=0xDFFF => return Err("A LONE SURROGATE"),
                                c => c,
                            };
                            out.push(char::from_u32(code).ok_or("A MALFORMED \\u ESCAPE")?);
                        }
                        _ => return Err("AN UNKNOWN ESCAPE"),
                    }
                }
                _ => return Err("A CONTROL CHARACTER IN A STRING"),
            }
        }
    }
}
