//! The two ends. The master sends a request and waits for its answer, sending it again a bounded number of times; the
//! slave answers each request once and repeats its last answer when the same request comes again (its answer was
//! lost), so a request is never carried out twice. Only the master starts an exchange.
use crate::frame::{self, Decoder, Header, MAX_PAYLOAD, MAX_WIRE};

/// Kinds of frame. A request's answer is its kind with `REPLY` set; `ERROR` answers a request that failed.
pub mod kind {
    pub const HELLO: u8 = 1;
    pub const POLL: u8 = 2;
    /// The slave's machine restarts, after its answer (800-LNK-0012).
    pub const REBOOT: u8 = 3;
    pub const PUT_OPEN: u8 = 10;
    pub const PUT_DATA: u8 = 11;
    pub const PUT_CLOSE: u8 = 12;
    pub const GET_OPEN: u8 = 13;
    pub const GET_DATA: u8 = 14;
    pub const GET_CLOSE: u8 = 15;
    pub const LIST: u8 = 16;
    pub const STAT: u8 = 17;
    pub const DELETE: u8 = 18;
    /// Files in directories (800-LNK-0010).
    pub const MKDIR: u8 = 19;
    pub const MOVE: u8 = 21;
    pub const RMDIR: u8 = 22;
    pub const LIST_DIR: u8 = 23;
    pub const CONSOLE: u8 = 20;
    pub const LOG: u8 = 30;
    pub const INPUT: u8 = 40;
    /// Programs the slave starts for the master (800-LNK-0009).
    pub const RUN: u8 = 50;
    pub const OUTPUT: u8 = 51;
    pub const STOP: u8 = 52;
    pub const RUNS: u8 = 53;
    /// A line for the slave's shell, whose answer is read as a run's output (800-LNK-0015).
    pub const COMMAND: u8 = 54;
    /// A service started again from a file the slave holds, or from its boot image (800-LNK-0016).
    pub const REPLACE: u8 = 55;
    pub const REPLY: u8 = 0x80;
    pub const ERROR: u8 = 0xFF;
}

/// Channels: what a frame belongs to.
pub mod channel {
    pub const CONTROL: u8 = 0;
    pub const CONSOLE: u8 = 1;
    pub const FILES: u8 = 2;
    pub const LOG: u8 = 3;
    pub const INPUT: u8 = 4;
    pub const PROGRAMS: u8 = 5;
}

/// Why an exchange failed; `Refused` carries the other end's code (`code`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Line, Timeout, TooBig, Refused(u8), Protocol }

/// The codes an `ERROR` frame carries.
pub mod code {
    pub const NOT_FOUND: u8 = 1;
    pub const DENIED: u8 = 2;
    pub const IO: u8 = 3;
    pub const MISMATCH: u8 = 4;
    pub const BAD_REQUEST: u8 = 5;
    pub const BUSY: u8 = 6;
    pub const UNSUPPORTED: u8 = 7;
    pub const NOT_EMPTY: u8 = 8;
    pub const EXISTS: u8 = 9;
}

/// A byte stream to the other end, and a clock.
pub trait Line {
    /// Sends all of `bytes`; false when the line failed.
    fn write(&mut self, bytes: &[u8]) -> bool;
    /// Bytes received into `out`, waiting at most `timeout_ms` for the first; 0 when none came.
    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize;
    /// Milliseconds on a clock that only goes forward.
    fn now_ms(&mut self) -> u64;
    /// Before the slave answers: time for a half-duplex line to turn round (RS-485 converters switch direction after
    /// the last byte). Nothing by default.
    fn turnaround(&mut self) {}
}

/// What counts of an end's traffic.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats { pub requests: u32, pub retries: u32, pub dropped: u32 }

pub struct Master<L: Line> {
    pub line: L,
    decoder: Decoder,
    seq: u16,
    wire: [u8; MAX_WIRE],
    chunk: [u8; 256],
    /// How long one try waits for the answer, and how many tries.
    pub timeout_ms: u32,
    pub tries: u32,
    pub stats: Stats,
    /// The slave this master asks on a shared line (800-LNK-0026): 1 to `frame::ADDRESS_MAX` in version 2 frames, or 0
    /// for a line with one slave (version 1, as before).
    pub address: u8,
}

impl<L: Line> Master<L> {
    /// The first sequence number comes from the clock, so a restarted master's requests do not look like its old ones.
    pub fn new(mut line: L) -> Self {
        let seq = line.now_ms() as u16;
        Self { line, decoder: Decoder::new(), seq, wire: [0; MAX_WIRE], chunk: [0; 256], timeout_ms: 500, tries: 8, stats: Stats::default(), address: 0 }
    }

    /// Sends a request and waits for its answer: the answer's payload in `reply`, and its length.
    pub fn call(&mut self, channel: u8, kind: u8, payload: &[u8], reply: &mut [u8]) -> Result<usize, Error> {
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;
        let n = frame::encode(&Header { address: self.address, channel, kind, flags: 0, seq }, payload, &mut self.wire).ok_or(Error::TooBig)?;
        self.stats.requests += 1;
        for attempt in 0..self.tries {
            if attempt > 0 { self.stats.retries += 1; }
            if !self.line.write(&self.wire[..n]) { return Err(Error::Line); }
            let end = self.line.now_ms() + self.timeout_ms as u64;
            loop {
                let now = self.line.now_ms();
                if now >= end { break; }
                let got = self.line.read(&mut self.chunk, (end - now) as u32);
                for i in 0..got {
                    let Some(h) = self.decoder.push(self.chunk[i]) else { continue };
                    // A request is this end's own echo (some RS-485 converters hear what they send); an answer to an earlier
                    // request is stale; one from another slave's address is not this exchange's.
                    if h.kind & kind::REPLY == 0 || h.seq != seq || h.channel != channel || h.address != self.address { continue; }
                    let p = self.decoder.payload();
                    if h.kind == kind::ERROR { return Err(Error::Refused(p.first().copied().unwrap_or(0))); }
                    if h.kind != kind | kind::REPLY { return Err(Error::Protocol); }
                    let out = reply.get_mut(..p.len()).ok_or(Error::TooBig)?;
                    out.copy_from_slice(p);
                    return Ok(p.len());
                }
            }
        }
        self.stats.dropped = self.decoder.dropped;
        Err(Error::Timeout)
    }
}

/// What the slave does with a request: the answer's payload in `reply` and its length, or an error code.
pub trait Service {
    fn handle(&mut self, channel: u8, kind: u8, payload: &[u8], reply: &mut [u8]) -> Result<usize, u8>;
}

pub struct Slave<L: Line> {
    pub line: L,
    decoder: Decoder,
    /// The last request answered: channel, kind, sequence and CRC-32C of its payload.
    last: Option<(u8, u8, u16, u32)>,
    answer: [u8; MAX_WIRE],
    answer_len: usize,
    chunk: [u8; 256],
    payload: [u8; MAX_PAYLOAD],
    reply: [u8; MAX_PAYLOAD],
    pub stats: Stats,
    /// This slave's address on a shared line (800-LNK-0026): it answers only version 2 frames to it. 0, the default, is
    /// a line with one slave: it answers only version 1 frames, so a master that addresses others never hears it.
    pub address: u8,
}

impl<L: Line> Slave<L> {
    pub fn new(line: L) -> Self {
        Self { line, decoder: Decoder::new(), last: None, answer: [0; MAX_WIRE], answer_len: 0, chunk: [0; 256], payload: [0; MAX_PAYLOAD],
               reply: [0; MAX_PAYLOAD], stats: Stats::default(), address: 0 }
    }

    /// Reads what the line has, waiting at most `timeout_ms`, and answers each whole request; how many it answered.
    pub fn serve(&mut self, service: &mut dyn Service, timeout_ms: u32) -> Result<u32, Error> {
        let got = self.line.read(&mut self.chunk, timeout_ms);
        let mut answered = 0;
        for i in 0..got {
            let Some(h) = self.decoder.push(self.chunk[i]) else { continue };
            // An answer is this end's own echo, or another slave's on a shared line; a request to another address is
            // not this slave's.
            if h.kind & kind::REPLY != 0 || h.address != self.address { continue; }
            self.stats.requests += 1;
            let p = self.decoder.payload();
            let id = (h.channel, h.kind, h.seq, crate::crc::crc32c(p));
            if self.last == Some(id) {
                // The same request again: its answer was lost, so the answer goes again, the request is not redone.
                self.stats.retries += 1;
            } else {
                let len = p.len();
                self.payload[..len].copy_from_slice(p);
                let (k, n) = match service.handle(h.channel, h.kind, &self.payload[..len], &mut self.reply) {
                    Ok(n) => (h.kind | kind::REPLY, n.min(MAX_PAYLOAD)),
                    Err(code) => { self.reply[0] = code; (kind::ERROR, 1) }
                };
                self.answer_len = frame::encode(&Header { address: self.address, channel: h.channel, kind: k, flags: 0, seq: h.seq }, &self.reply[..n], &mut self.answer).ok_or(Error::TooBig)?;
                self.last = Some(id);
            }
            self.line.turnaround();
            if !self.line.write(&self.answer[..self.answer_len]) { return Err(Error::Line); }
            answered += 1;
        }
        self.stats.dropped = self.decoder.dropped;
        Ok(answered)
    }
}
