//! Server-Sent Events (`text/event-stream`, the WHATWG HTML standard, section 9.2), read from a body as it arrives
//! (000-NET-0047): every language model's API streams its answer this way. The bytes are fed in pieces of any size;
//! each event is handed to the caller whole, with a stated limit on a line and on an event's data. Nothing here repeats
//! a request or reconnects: `retry` and the last event's id are the caller's to use (MC-6.6).
use alloc::string::String;
use alloc::vec::Vec;

/// An event: its type (`message` unless the stream named one), its data (the `data` lines joined by line feeds) and
/// the last event id the stream set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event { pub kind: String, pub data: String, pub id: String }

/// Why a stream was given up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { TooLarge }

/// A stream being read.
pub struct Stream { limit: usize, line: Vec<u8>, kind: String, data: String, id: String, retry: Option<u64>, after_cr: bool, started: bool }

impl Stream {
    /// A stream whose lines and events' data are at most `limit` bytes each.
    pub fn new(limit: usize) -> Self {
        Stream { limit, line: Vec::new(), kind: String::new(), data: String::new(), id: String::new(), retry: None, after_cr: false, started: false }
    }

    /// The reconnection time the stream asked for, in milliseconds.
    pub fn retry(&self) -> Option<u64> { self.retry }

    /// The last event id the stream set.
    pub fn last_id(&self) -> &str { &self.id }

    /// Reads `bytes`, handing each complete event to `event`; false when `event` asked to stop (the rest of `bytes` is
    /// left unread). A line or an event's data over the limit gives up the stream.
    pub fn feed(&mut self, bytes: &[u8], event: &mut dyn FnMut(Event) -> bool) -> Result<bool, Error> {
        for &byte in bytes {
            // A line ends with CR LF, LF or CR; an LF right after a CR ends nothing more.
            if core::mem::take(&mut self.after_cr) && byte == b'\n' { continue; }
            if byte == b'\r' || byte == b'\n' {
                self.after_cr = byte == b'\r';
                let line = core::mem::take(&mut self.line);
                if !self.line_ended(&line, event)? { return Ok(false); }
                continue;
            }
            if self.line.len() == self.limit { return Err(Error::TooLarge); }
            self.line.push(byte);
        }
        Ok(true)
    }

    // One line: a blank one dispatches the event, a colon first is a comment, else a field and its value.
    fn line_ended(&mut self, raw: &[u8], event: &mut dyn FnMut(Event) -> bool) -> Result<bool, Error> {
        // A byte order mark at the start of the stream is dropped.
        let raw = if !self.started && raw.starts_with(&[0xEF, 0xBB, 0xBF]) { &raw[3..] } else { raw };
        self.started = true;
        if raw.is_empty() { return Ok(self.dispatch(event)); }
        if raw[0] == b':' { return Ok(true); }
        let line = String::from_utf8_lossy(raw);
        let (name, value) = match line.split_once(':') { Some((n, v)) => (n, v.strip_prefix(' ').unwrap_or(v)), None => (&line[..], "") };
        match name {
            "event" => { self.kind.clear(); self.kind.push_str(value); }
            "data" => {
                if self.data.len() + value.len() + 1 > self.limit { return Err(Error::TooLarge); }
                self.data.push_str(value);
                self.data.push('\n');
            }
            "id" if !value.contains('\0') => { self.id.clear(); self.id.push_str(value); }
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => self.retry = value.parse().ok().or(self.retry),
            _ => {}
        }
        Ok(true)
    }

    // An event without data is not dispatched; either way its type and data start again.
    fn dispatch(&mut self, event: &mut dyn FnMut(Event) -> bool) -> bool {
        let mut data = core::mem::take(&mut self.data);
        let kind = core::mem::take(&mut self.kind);
        if data.is_empty() { return true; }
        data.pop();
        event(Event { kind: if kind.is_empty() { String::from("message") } else { kind }, data, id: self.id.clone() })
    }
}
