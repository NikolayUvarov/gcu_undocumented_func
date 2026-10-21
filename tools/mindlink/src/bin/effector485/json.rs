//! JSON for effector485's requests, answers and state file: a value, a parser and a writer, standard library only.
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Value { Null, Bool(bool), Num(f64), Str(String), Arr(Vec<Value>), Obj(Vec<(String, Value)>) }

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self { Value::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v), _ => None }
    }
    pub fn str(&self, key: &str) -> Option<&str> { match self.get(key) { Some(Value::Str(s)) => Some(s), _ => None } }
    pub fn num(&self, key: &str) -> Option<f64> { match self.get(key) { Some(Value::Num(n)) => Some(*n), _ => None } }
    pub fn bool(&self, key: &str) -> Option<bool> { match self.get(key) { Some(Value::Bool(b)) => Some(*b), _ => None } }

    /// Sets `key` in an object, replacing its value.
    pub fn set(&mut self, key: &str, value: Value) {
        if let Value::Obj(fields) = self {
            match fields.iter_mut().find(|(k, _)| k == key) { Some(f) => f.1 = value, None => fields.push((key.to_string(), value)) }
        }
    }

    pub fn text(&self) -> String { let mut s = String::new(); self.write(&mut s); s }

    fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => { let _ = write!(out, "{}", *n as i64); }
            Value::Num(n) => { let _ = write!(out, "{n}"); }
            Value::Str(s) => quote(s, out),
            Value::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() { if i > 0 { out.push(','); } v.write(out); }
                out.push(']');
            }
            Value::Obj(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() { if i > 0 { out.push(','); } quote(k, out); out.push(':'); v.write(out); }
                out.push('}');
            }
        }
    }
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => { let _ = write!(out, "\\u{:04x}", c as u32); }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// An object from its fields.
pub fn obj(fields: Vec<(&str, Value)>) -> Value { Value::Obj(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect()) }
pub fn s(text: &str) -> Value { Value::Str(text.to_string()) }

pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser { b: text.as_bytes(), at: 0, depth: 0 };
    let v = p.value()?;
    p.space();
    if p.at != p.b.len() { return Err(format!("JSON: text after the value at byte {}", p.at)); }
    Ok(v)
}

struct Parser<'a> { b: &'a [u8], at: usize, depth: u32 }

impl Parser<'_> {
    fn space(&mut self) { while self.b.get(self.at).is_some_and(|c| c.is_ascii_whitespace()) { self.at += 1; } }
    fn fail<T>(&self, what: &str) -> Result<T, String> { Err(format!("JSON: {what} at byte {}", self.at)) }
    fn eat(&mut self, word: &str) -> bool {
        if self.b[self.at..].starts_with(word.as_bytes()) { self.at += word.len(); true } else { false }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.space();
        self.depth += 1;
        if self.depth > 64 { return self.fail("nested too deep"); }
        let v = match self.b.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                self.space();
                if !self.eat("}") {
                    loop {
                        self.space();
                        let Value::Str(k) = self.string()? else { unreachable!() };
                        self.space();
                        if !self.eat(":") { return self.fail("':' expected"); }
                        let v = self.value()?;
                        fields.push((k, v));
                        self.space();
                        if self.eat("}") { break; }
                        if !self.eat(",") { return self.fail("',' or '}' expected"); }
                    }
                }
                Value::Obj(fields)
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if !self.eat("]") {
                    loop {
                        items.push(self.value()?);
                        self.space();
                        if self.eat("]") { break; }
                        if !self.eat(",") { return self.fail("',' or ']' expected"); }
                    }
                }
                Value::Arr(items)
            }
            Some(b'"') => self.string()?,
            Some(b't') if self.eat("true") => Value::Bool(true),
            Some(b'f') if self.eat("false") => Value::Bool(false),
            Some(b'n') if self.eat("null") => Value::Null,
            Some(c) if *c == b'-' || c.is_ascii_digit() => {
                let start = self.at;
                while self.b.get(self.at).is_some_and(|c| matches!(c, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')) { self.at += 1; }
                let text = std::str::from_utf8(&self.b[start..self.at]).unwrap_or("");
                Value::Num(text.parse().map_err(|_| format!("JSON: bad number at byte {start}"))?)
            }
            _ => return self.fail("a value expected"),
        };
        self.depth -= 1;
        Ok(v)
    }

    fn string(&mut self) -> Result<Value, String> {
        if !self.eat("\"") { return self.fail("'\"' expected"); }
        let mut out = String::new();
        loop {
            let Some(&c) = self.b.get(self.at) else { return self.fail("unterminated string") };
            self.at += 1;
            match c {
                b'"' => return Ok(Value::Str(out)),
                b'\\' => {
                    let Some(&e) = self.b.get(self.at) else { return self.fail("unterminated escape") };
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'), b'\\' => out.push('\\'), b'/' => out.push('/'), b'n' => out.push('\n'),
                        b'r' => out.push('\r'), b't' => out.push('\t'), b'b' => out.push('\u{8}'), b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = |p: &mut Self| -> Result<u32, String> {
                                let h = p.b.get(p.at..p.at + 4).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u32::from_str_radix(h, 16).ok());
                                p.at += 4;
                                h.ok_or_else(|| format!("JSON: bad \\u escape at byte {}", p.at))
                            };
                            let mut code = hex(self)?;
                            // A surrogate pair stands for one character.
                            if (0xD800..0xDC00).contains(&code) && self.eat("\\u") { code = 0x10000 + ((code - 0xD800) << 10) + (hex(self)? - 0xDC00); }
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => return self.fail("bad escape"),
                    }
                }
                _ => {
                    // UTF-8 passes through whole: the input is a &str.
                    let start = self.at - 1;
                    let mut end = self.at;
                    while end < self.b.len() && self.b[end] & 0xC0 == 0x80 { end += 1; }
                    out.push_str(std::str::from_utf8(&self.b[start..end]).map_err(|_| format!("JSON: bad UTF-8 at byte {start}"))?);
                    self.at = end;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let text = r#"{"a":1,"b":[true,false,null],"c":"x\"y\\z\n\u00e9\ud83d\ude00","d":{"e":-2.5}}"#;
        let v = parse(text).unwrap();
        assert_eq!(v.str("c"), Some("x\"y\\z\né😀"));
        assert_eq!(v.get("d").unwrap().num("e"), Some(-2.5));
        assert_eq!(parse(&v.text()).unwrap(), v);
        for bad in ["{", "[1,", "{\"a\" 1}", "\"\\x\"", "1 2", &"[".repeat(100)] { assert!(parse(bad).is_err(), "{bad}"); }
    }
}
