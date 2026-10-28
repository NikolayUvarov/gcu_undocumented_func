//! A bounded JSON writer without allocation for the agent's requests (750-EFF-0004); reading is `parse`'s (750-NET-0021).
//! It writes only valid JSON: strings escaped, nesting checked, and a body that does not fit is an error, never cut.

pub const DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Full, Nesting, Order }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame { Object, Array }

/// Writes one JSON value into `buf`. Inside an object every value needs a `key` first.
pub struct Writer<'a> {
    buf: &'a mut [u8],
    len: usize,
    stack: [Frame; DEPTH],
    depth: usize,
    first: bool,
    keyed: bool,
    done: bool,
}

impl<'a> Writer<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, len: 0, stack: [Frame::Object; DEPTH], depth: 0, first: true, keyed: false, done: false }
    }

    /// The text written, once the outermost value is closed.
    pub fn finish(self) -> Result<&'a [u8], Error> {
        if self.depth != 0 || !self.done { return Err(Error::Order); }
        let Writer { buf, len, .. } = self;
        Ok(&buf[..len])
    }

    fn bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        let end = self.len.checked_add(b.len()).ok_or(Error::Full)?;
        if end > self.buf.len() { return Err(Error::Full); }
        self.buf[self.len..end].copy_from_slice(b);
        self.len = end;
        Ok(())
    }

    // Before a value: a comma between array items, a key inside an object, nothing after the outermost value.
    fn value(&mut self) -> Result<(), Error> {
        if self.done { return Err(Order); }
        match self.stack[..self.depth].last() {
            None => {}
            Some(Frame::Object) => { if !self.keyed { return Err(Order); } self.keyed = false; }
            Some(Frame::Array) => { if !self.first { self.bytes(b",")?; } self.first = false; }
        }
        Ok(())
    }

    fn closed(&mut self) { if self.depth == 0 { self.done = true; } }

    fn quoted(&mut self, s: &str) -> Result<(), Error> {
        self.bytes(b"\"")?;
        for c in s.chars() {
            let mut tmp = [0u8; 6];
            let out: &[u8] = match c {
                '"' => b"\\\"",
                '\\' => b"\\\\",
                '\n' => b"\\n",
                '\r' => b"\\r",
                '\t' => b"\\t",
                c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    let v = c as u32;
                    tmp = [b'\\', b'u', b'0', b'0', HEX[(v >> 4) as usize & 15], HEX[v as usize & 15]];
                    &tmp
                }
                c => c.encode_utf8(&mut tmp).as_bytes(),
            };
            self.bytes(out)?;
        }
        self.bytes(b"\"")
    }

    pub fn object(&mut self) -> Result<(), Error> { self.open(Frame::Object, b"{") }
    pub fn array(&mut self) -> Result<(), Error> { self.open(Frame::Array, b"[") }

    fn open(&mut self, frame: Frame, b: &[u8]) -> Result<(), Error> {
        self.value()?;
        if self.depth == DEPTH { return Err(Error::Nesting); }
        self.bytes(b)?;
        self.stack[self.depth] = frame;
        self.depth += 1;
        self.first = true;
        Ok(())
    }

    pub fn end(&mut self) -> Result<(), Error> {
        let Some(&frame) = self.stack[..self.depth].last() else { return Err(Order) };
        if frame == Frame::Object && self.keyed { return Err(Order); }
        self.bytes(if frame == Frame::Object { b"}" } else { b"]" })?;
        self.depth -= 1;
        self.first = false;
        self.closed();
        Ok(())
    }

    pub fn key(&mut self, k: &str) -> Result<(), Error> {
        if self.stack[..self.depth].last() != Some(&Frame::Object) || self.keyed { return Err(Order); }
        if !self.first { self.bytes(b",")?; }
        self.first = false;
        self.quoted(k)?;
        self.bytes(b":")?;
        self.keyed = true;
        Ok(())
    }

    pub fn string(&mut self, s: &str) -> Result<(), Error> { self.value()?; self.quoted(s)?; self.closed(); Ok(()) }
    pub fn boolean(&mut self, v: bool) -> Result<(), Error> { self.value()?; self.bytes(if v { b"true" } else { b"false" })?; self.closed(); Ok(()) }
    pub fn null(&mut self) -> Result<(), Error> { self.value()?; self.bytes(b"null")?; self.closed(); Ok(()) }

    pub fn unsigned(&mut self, mut v: u64) -> Result<(), Error> {
        self.value()?;
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop { i -= 1; digits[i] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
        self.bytes(&digits[i..])?;
        self.closed();
        Ok(())
    }

    pub fn signed(&mut self, v: i64) -> Result<(), Error> {
        if v >= 0 { return self.unsigned(v as u64); }
        self.value()?;
        self.bytes(b"-")?;
        // Write the magnitude without the value bookkeeping, which was done.
        let mut m = v.unsigned_abs();
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop { i -= 1; digits[i] = b'0' + (m % 10) as u8; m /= 10; if m == 0 { break; } }
        self.bytes(&digits[i..])?;
        self.closed();
        Ok(())
    }

    /// `thousandths` / 1000 as a decimal number with three places (`12.345`), for rates and percentages.
    pub fn decimal(&mut self, thousandths: u64) -> Result<(), Error> {
        self.value()?;
        let (mut v, mut digits) = (thousandths / 1000, [0u8; 24]);
        let mut i = digits.len() - 4;
        let fraction = thousandths % 1000;
        digits[i..].copy_from_slice(&[b'.', b'0' + (fraction / 100) as u8, b'0' + (fraction / 10 % 10) as u8, b'0' + (fraction % 10) as u8]);
        loop { i -= 1; digits[i] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
        self.bytes(&digits[i..])?;
        self.closed();
        Ok(())
    }

    /// `"key": "value"` inside an object.
    pub fn field_str(&mut self, k: &str, v: &str) -> Result<(), Error> { self.key(k)?; self.string(v) }
    pub fn field_u64(&mut self, k: &str, v: u64) -> Result<(), Error> { self.key(k)?; self.unsigned(v) }
    pub fn field_bool(&mut self, k: &str, v: bool) -> Result<(), Error> { self.key(k)?; self.boolean(v) }
    pub fn field_decimal(&mut self, k: &str, thousandths: u64) -> Result<(), Error> { self.key(k)?; self.decimal(thousandths) }
}

use Error::Order;
