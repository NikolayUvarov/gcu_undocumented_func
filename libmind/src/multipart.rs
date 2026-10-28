//! `multipart/form-data` request bodies (RFC 7578), sent as a `mind::http::Source` (000-NET-0047): fields given whole,
//! and files streamed from a source of a stated length (Telegram's `sendDocument`, a form's upload). The boundary is
//! made from 16 random bytes the caller gives, and the body is checked not to hold it: a field before anything is sent,
//! a file as it streams (the request then fails with `Body`). The length is known before the first byte.
use crate::http::{Error, Source};

/// A part of the body: a field, or a file read from `source`, which must give exactly `length` bytes.
pub enum Part<'a> {
    Field { name: &'a str, value: &'a [u8] },
    File { name: &'a str, filename: &'a str, media: &'a str, length: u64, source: &'a mut dyn Source },
}

/// The boundary's length: `MIND-` and 32 hexadecimal digits.
pub const BOUNDARY_LEN: usize = 37;

/// A body being written: the parts, the part and the place in it.
pub struct Multipart<'p, 'a> {
    parts: &'p mut [Part<'a>],
    boundary: [u8; BOUNDARY_LEN],
    /// The part being written, and where: in its head (`at` bytes of it), its body, or its closing line.
    part: usize,
    stage: Stage,
    at: u64,
    /// How far the delimiter `--boundary` is matched in the file streaming now, and the delimiter's failure table.
    matched: usize,
    fail: [u8; DELIMITER_LEN],
}

const DELIMITER_LEN: usize = 2 + BOUNDARY_LEN;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage { Head, Body, Close, End, Done }

// A name or file name goes in a quoted string: no quote, backslash or control character.
fn quotable(text: &str) -> bool { !text.is_empty() && text.len() <= 256 && text.bytes().all(|b| b >= 0x20 && b != 0x7F && b != b'"' && b != b'\\') }

impl<'p, 'a> Multipart<'p, 'a> {
    /// A body of `parts`, its boundary made from `random`. Refuses a name, file name or media type that cannot be
    /// written, and a field that holds the boundary.
    pub fn new(parts: &'p mut [Part<'a>], random: [u8; 16]) -> Result<Self, Error> {
        let mut boundary = [0u8; BOUNDARY_LEN];
        boundary[..5].copy_from_slice(b"MIND-");
        for (i, byte) in random.iter().enumerate() {
            boundary[5 + 2 * i] = b"0123456789abcdef"[(byte >> 4) as usize];
            boundary[6 + 2 * i] = b"0123456789abcdef"[(byte & 15) as usize];
        }
        for part in parts.iter() {
            let fine = match part {
                Part::Field { name, value } => quotable(name) && !value.windows(BOUNDARY_LEN).any(|w| w == boundary),
                Part::File { name, filename, media, .. } => quotable(name) && quotable(filename)
                    && media.split('/').count() == 2 && media.len() <= 64 && media.bytes().all(|b| b.is_ascii_graphic()),
            };
            if !fine { return Err(Error::Body); }
        }
        let mut multipart = Multipart { parts, boundary, part: 0, stage: Stage::Head, at: 0, matched: 0, fail: [0; DELIMITER_LEN] };
        // Knuth-Morris-Pratt: the longest proper prefix of the delimiter that ends each of its prefixes.
        let mut k = 0;
        for i in 1..DELIMITER_LEN {
            while k > 0 && multipart.delimiter(i) != multipart.delimiter(k) { k = multipart.fail[k - 1] as usize; }
            if multipart.delimiter(i) == multipart.delimiter(k) { k += 1; }
            multipart.fail[i] = k as u8;
        }
        Ok(multipart)
    }

    // Byte `i` of the delimiter `--boundary`.
    fn delimiter(&self, i: usize) -> u8 { if i < 2 { b'-' } else { self.boundary[i - 2] } }

    /// The boundary.
    pub fn boundary(&self) -> &str { core::str::from_utf8(&self.boundary).unwrap_or("") }

    /// The body's length in bytes.
    pub fn length(&self) -> u64 {
        let parts: u64 = (0..self.parts.len()).map(|i| self.head(i, &mut [0u8; 0]).1 as u64 + self.body_length(i) + 2).sum();
        parts + 2 + BOUNDARY_LEN as u64 + 4
    }

    fn body_length(&self, i: usize) -> u64 { match &self.parts[i] { Part::Field { value, .. } => value.len() as u64, Part::File { length, .. } => *length } }

    // Part `i`'s head into `out` from byte `from`: how many bytes went, and the head's whole length.
    fn head(&self, i: usize, out: &mut [u8]) -> (usize, usize) { self.head_from(i, 0, out) }

    fn head_from(&self, i: usize, from: usize, out: &mut [u8]) -> (usize, usize) {
        let pieces: [&[u8]; 9] = match &self.parts[i] {
            Part::Field { name, .. } => [b"--", &self.boundary, b"\r\nContent-Disposition: form-data; name=\"", name.as_bytes(), b"\"\r\n\r\n", b"", b"", b"", b""],
            Part::File { name, filename, media, .. } => [b"--", &self.boundary, b"\r\nContent-Disposition: form-data; name=\"", name.as_bytes(), b"\"; filename=\"",
                                                           filename.as_bytes(), b"\"\r\nContent-Type: ", media.as_bytes(), b"\r\n\r\n"],
        };
        copy_from(&pieces, from, out)
    }
}

// Bytes `from` on of the pieces joined, into `out`: how many went, and the joined length.
fn copy_from(pieces: &[&[u8]], from: usize, out: &mut [u8]) -> (usize, usize) {
    let (mut position, mut written) = (0, 0);
    for piece in pieces {
        let end = position + piece.len();
        if end > from && written < out.len() {
            let start = from.max(position) - position;
            let n = (piece.len() - start).min(out.len() - written);
            out[written..written + n].copy_from_slice(&piece[start..start + n]);
            written += n;
        }
        position = end;
    }
    (written, position)
}

impl Source for Multipart<'_, '_> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let mut written = 0;
        while written < buffer.len() && self.stage != Stage::Done {
            let out = &mut buffer[written..];
            match self.stage {
                Stage::Head => {
                    let (n, whole) = self.head_from(self.part, self.at as usize, out);
                    written += n;
                    self.at += n as u64;
                    if self.at as usize == whole { self.stage = Stage::Body; self.at = 0; self.matched = 0; }
                }
                Stage::Body => {
                    let left = (self.body_length(self.part) - self.at) as usize;
                    let want = left.min(out.len());
                    let n = match &mut self.parts[self.part] {
                        Part::Field { value, .. } => { out[..want].copy_from_slice(&value[self.at as usize..self.at as usize + want]); want }
                        Part::File { source, .. } => {
                            let n = source.read(&mut out[..want])?.min(want);
                            if n == 0 && want > 0 { return Err(Error::Body); }
                            n
                        }
                    };
                    // The delimiter must not occur in a file: matched as it streams, across reads.
                    if matches!(self.parts[self.part], Part::File { .. }) {
                        for &byte in &out[..n] {
                            while self.matched > 0 && byte != self.delimiter(self.matched) { self.matched = self.fail[self.matched - 1] as usize; }
                            if byte == self.delimiter(self.matched) { self.matched += 1; }
                            if self.matched == DELIMITER_LEN { return Err(Error::Body); }
                        }
                    }
                    written += n;
                    self.at += n as u64;
                    if self.at == self.body_length(self.part) { self.stage = Stage::Close; self.at = 0; }
                }
                Stage::Close => {
                    let (n, whole) = copy_from(&[b"\r\n"], self.at as usize, out);
                    written += n;
                    self.at += n as u64;
                    if self.at as usize == whole {
                        self.at = 0;
                        self.part += 1;
                        self.stage = if self.part == self.parts.len() { Stage::End } else { Stage::Head };
                    }
                }
                Stage::End => {
                    let (n, whole) = copy_from(&[b"--", &self.boundary, b"--\r\n"], self.at as usize, out);
                    written += n;
                    self.at += n as u64;
                    if self.at as usize == whole { self.stage = Stage::Done; }
                }
                Stage::Done => {}
            }
        }
        Ok(written)
    }
}
