//! A response's body decoded by its content coding as it arrives (000-NET-0047): identity, gzip (one member or
//! several) and deflate (zlib, or the raw DEFLATE some servers send instead). The decoded bytes go to the caller's sink
//! in pieces, never held whole, and stop at the caller's limit, so a small body that inflates without end (a bomb)
//! fails there with `TooLarge`. A coding other than these fails with `Encoding`, as does a body that does not decode.
use crate::http::{Encoding, Error, Response, Transport};
use crate::inflate;

// The body read a piece at a time, as the decoder asks for bytes; a failed read is kept for the caller.
struct Body<'r, 'c, T: Transport> { response: &'r mut Response<'c, T>, buffer: [u8; 4096], at: usize, end: usize, failed: Option<Error> }

impl<T: Transport> Body<'_, '_, T> {
    // Whether a byte is left, reading the next piece if need be.
    fn more(&mut self) -> bool {
        if self.at < self.end { return true; }
        if self.failed.is_some() { return false; }
        match self.response.read(&mut self.buffer) {
            Ok(n) => { self.at = 0; self.end = n; n > 0 }
            Err(error) => { self.failed = Some(error); false }
        }
    }
}

impl<T: Transport> inflate::Input for Body<'_, '_, T> {
    fn byte(&mut self) -> Option<u8> {
        if !self.more() { return None; }
        self.at += 1;
        Some(self.buffer[self.at - 1])
    }
}

/// Reads `response`'s body to its end, decoded by its content coding, into `sink` (which may return false to stop
/// early); at most `limit` decoded bytes. Returns how many it gave.
pub fn decode<T: Transport>(response: &mut Response<'_, T>, limit: u64, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<u64, Error> {
    let encoding = response.head.encoding;
    let (mut given, mut over, mut stopped) = (0u64, false, false);
    let mut limited = |piece: &[u8]| -> bool {
        if given + piece.len() as u64 > limit { over = true; return false; }
        given += piece.len() as u64;
        if !sink(piece) { stopped = true; return false; }
        true
    };
    let mut body = Body { response, buffer: [0; 4096], at: 0, end: 0, failed: None };
    let decoded: Result<(), &'static str> = match encoding {
        Encoding::Other => return Err(Error::Encoding),
        Encoding::Identity => {
            while body.more() {
                let (at, end) = (body.at, body.end);
                body.at = end;
                if !limited(&body.buffer[at..end]) { break; }
            }
            Ok(())
        }
        Encoding::Gzip => {
            // Members one after another make one body (RFC 1952 2.2).
            let mut result = Ok(());
            while body.more() {
                match inflate::gzip_from(&mut body, &mut limited) {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => { result = Err(e); break; }
                }
            }
            result
        }
        Encoding::Deflate => {
            // zlib as RFC 9110 names it, or the raw stream some servers send: told apart by the first two bytes.
            let (first, second) = (inflate::Input::byte(&mut body), inflate::Input::byte(&mut body));
            match (first, second) {
                (Some(a), Some(b)) => {
                    let mut input = Prefixed { first: [a, b], given: 0, rest: &mut body };
                    let decoded = if inflate::zlib_header(a, b) { inflate::zlib_from(&mut input, &mut limited) } else { inflate::inflate_from(&mut input, &mut limited) };
                    decoded.map(|_| ())
                }
                (None, _) => Ok(()),
                _ => Err("the stream ends early"),
            }
        }
    };
    if let Some(error) = body.failed { return Err(error); }
    if over { return Err(Error::TooLarge); }
    decoded.map_err(|_| Error::Encoding)?;
    // What follows a coded stream is dropped, so the connection can carry the next request; a sink that stopped
    // leaves the rest unread.
    if !stopped { while body.more() { body.at = body.end; } }
    if let Some(error) = body.failed { return Err(error); }
    Ok(given)
}

// Two bytes already read, then the rest.
struct Prefixed<'b, I: inflate::Input> { first: [u8; 2], given: usize, rest: &'b mut I }

impl<I: inflate::Input> inflate::Input for Prefixed<'_, I> {
    fn byte(&mut self) -> Option<u8> {
        if self.given < 2 { self.given += 1; return Some(self.first[self.given - 1]); }
        self.rest.byte()
    }
}
