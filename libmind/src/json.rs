//! A JSON reader (RFC 8259) without allocation, for bounded documents read in the parser service (251-STO-0014): a
//! cursor over the text that names the next value's kind, walks into objects and arrays, copies a string with its
//! escapes decoded into the caller's buffer, reads a non-negative integer, and skips any value. The text must be UTF-8;
//! nesting is at most `DEPTH`; anything else malformed is an error, never a guess.

/// The deepest nesting read.
pub const DEPTH: usize = 32;

/// The text is not JSON, or not of the shape asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;
pub type Result<T> = core::result::Result<T, Malformed>;

/// What the next value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Object, Array, String, Number, True, False, Null }

/// A cursor over one JSON text.
pub struct Reader<'a> { text: &'a [u8], at: usize, depth: usize, started: u64 }

impl<'a> Reader<'a> {
    /// A reader of `text`, which must be UTF-8.
    pub fn new(text: &'a [u8]) -> Result<Self> {
        core::str::from_utf8(text).map_err(|_| Malformed)?;
        Ok(Self { text, at: 0, depth: 0, started: 0 })
    }

    /// Where the cursor is, in bytes from the start (the next value's first byte after `peek`).
    pub fn position(&self) -> usize { self.at }

    fn space(&mut self) {
        while self.at < self.text.len() && matches!(self.text[self.at], b' ' | b'\t' | b'\n' | b'\r') { self.at += 1; }
    }

    fn byte(&mut self) -> Result<u8> { let b = *self.text.get(self.at).ok_or(Malformed)?; self.at += 1; Ok(b) }

    fn literal(&mut self, word: &[u8]) -> Result<()> {
        if self.text.get(self.at..self.at + word.len()) != Some(word) { return Err(Malformed); }
        self.at += word.len();
        Ok(())
    }

    /// The kind of the next value (whitespace before it is passed over).
    pub fn peek(&mut self) -> Result<Kind> {
        self.space();
        Ok(match *self.text.get(self.at).ok_or(Malformed)? {
            b'{' => Kind::Object, b'[' => Kind::Array, b'"' => Kind::String, b'-' | b'0'..=b'9' => Kind::Number,
            b't' => Kind::True, b'f' => Kind::False, b'n' => Kind::Null, _ => return Err(Malformed),
        })
    }

    fn open(&mut self, bracket: u8) -> Result<()> {
        self.space();
        if self.depth == DEPTH || self.byte()? != bracket { return Err(Malformed); }
        self.started &= !(1 << self.depth);
        self.depth += 1;
        Ok(())
    }

    // Before the next member or item of the innermost container: false at its closing bracket (consumed).
    fn more(&mut self, close: u8) -> Result<bool> {
        self.space();
        let level = self.depth - 1;
        if self.text.get(self.at) == Some(&close) { self.at += 1; self.depth -= 1; return Ok(false); }
        if self.started & (1 << level) != 0 {
            if self.byte()? != b',' { return Err(Malformed); }
            self.space();
        }
        self.started |= 1 << level;
        Ok(true)
    }

    /// Enters an object; `key` then gives its members.
    pub fn object(&mut self) -> Result<()> { self.open(b'{') }

    /// The next member's key, copied into `out`, with the colon after it passed; None at the object's end.
    pub fn key<'b>(&mut self, out: &'b mut [u8]) -> Result<Option<&'b [u8]>> {
        if self.depth == 0 || !self.more(b'}')? { return Ok(None); }
        let n = self.string_into(out)?;
        self.space();
        if self.byte()? != b':' { return Err(Malformed); }
        Ok(Some(&out[..n]))
    }

    /// Enters an array; `item` then says whether another value follows.
    pub fn array(&mut self) -> Result<()> { self.open(b'[') }

    /// Whether another item follows (its value is next); false at the array's end.
    pub fn item(&mut self) -> Result<bool> { if self.depth == 0 { return Ok(false); } self.more(b']') }

    /// A string value, its escapes decoded, copied into `out`.
    pub fn string<'b>(&mut self, out: &'b mut [u8]) -> Result<&'b [u8]> { let n = self.string_into(out)?; Ok(&out[..n]) }

    fn string_into(&mut self, out: &mut [u8]) -> Result<usize> {
        self.space();
        if self.byte()? != b'"' { return Err(Malformed); }
        let mut n = 0;
        let mut put = |bytes: &[u8], n: &mut usize| -> Result<()> {
            let end = *n + bytes.len();
            out.get_mut(*n..end).ok_or(Malformed)?.copy_from_slice(bytes);
            *n = end;
            Ok(())
        };
        loop {
            match self.byte()? {
                b'"' => return Ok(n),
                b'\\' => {
                    let c = match self.byte()? {
                        b'"' => '"', b'\\' => '\\', b'/' => '/', b'b' => '\u{8}', b'f' => '\u{c}', b'n' => '\n', b'r' => '\r', b't' => '\t',
                        b'u' => {
                            let high = self.hex4()?;
                            let code = match high {
                                0xD800..=0xDBFF => {
                                    self.literal(b"\\u")?;
                                    let low = self.hex4()?;
                                    if !(0xDC00..=0xDFFF).contains(&low) { return Err(Malformed); }
                                    0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
                                }
                                0xDC00..=0xDFFF => return Err(Malformed),
                                _ => high,
                            };
                            char::from_u32(code).ok_or(Malformed)?
                        }
                        _ => return Err(Malformed),
                    };
                    put(c.encode_utf8(&mut [0; 4]).as_bytes(), &mut n)?;
                }
                b if b < 0x20 => return Err(Malformed),
                b => put(&[b], &mut n)?,
            }
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        let mut v = 0;
        for _ in 0..4 { v = v << 4 | (self.byte()? as char).to_digit(16).ok_or(Malformed)?; }
        Ok(v)
    }

    // The number's text, checked against the grammar.
    fn number_text(&mut self) -> Result<&'a [u8]> {
        self.space();
        let start = self.at;
        let digits = |r: &mut Self| { let s = r.at; while r.text.get(r.at).is_some_and(u8::is_ascii_digit) { r.at += 1; } r.at - s };
        if self.text.get(self.at) == Some(&b'-') { self.at += 1; }
        match self.text.get(self.at) {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => { digits(self); }
            _ => return Err(Malformed),
        }
        if self.text.get(self.at) == Some(&b'.') { self.at += 1; if digits(self) == 0 { return Err(Malformed); } }
        if matches!(self.text.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.text.get(self.at), Some(b'+' | b'-')) { self.at += 1; }
            if digits(self) == 0 { return Err(Malformed); }
        }
        Ok(&self.text[start..self.at])
    }

    /// A number that is a non-negative integer without a fraction or an exponent, that fits 64 bits.
    pub fn integer(&mut self) -> Result<u64> {
        let text = self.number_text()?;
        if !text.iter().all(u8::is_ascii_digit) { return Err(Malformed); }
        text.iter().try_fold(0u64, |n, &d| n.checked_mul(10)?.checked_add((d - b'0') as u64)).ok_or(Malformed)
    }

    /// Passes over the next value, whatever it is, checking it.
    pub fn skip(&mut self) -> Result<()> {
        match self.peek()? {
            Kind::Object => {
                self.object()?;
                // Keys are checked but not kept.
                while self.more(b'}')? { self.skip_string()?; self.space(); if self.byte()? != b':' { return Err(Malformed); } self.skip()?; }
            }
            Kind::Array => { self.array()?; while self.item()? { self.skip()?; } }
            Kind::String => self.skip_string()?,
            Kind::Number => { self.number_text()?; }
            Kind::True => self.literal(b"true")?,
            Kind::False => self.literal(b"false")?,
            Kind::Null => self.literal(b"null")?,
        }
        Ok(())
    }

    // A string checked as `string` does, its characters not kept.
    fn skip_string(&mut self) -> Result<()> {
        self.space();
        if self.byte()? != b'"' { return Err(Malformed); }
        loop {
            match self.byte()? {
                b'"' => return Ok(()),
                b'\\' => match self.byte()? {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                    b'u' => {
                        let high = self.hex4()?;
                        if (0xD800..=0xDBFF).contains(&high) {
                            self.literal(b"\\u")?;
                            if !(0xDC00..=0xDFFF).contains(&self.hex4()?) { return Err(Malformed); }
                        } else if (0xDC00..=0xDFFF).contains(&high) { return Err(Malformed); }
                    }
                    _ => return Err(Malformed),
                },
                b if b < 0x20 => return Err(Malformed),
                _ => {}
            }
        }
    }

    /// Checks that only whitespace is left.
    pub fn end(&mut self) -> Result<()> { self.space(); if self.at == self.text.len() && self.depth == 0 { Ok(()) } else { Err(Malformed) } }
}
