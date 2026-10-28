//! `ping`'s command line and its tally (000-APP-0095): how many echo requests, how often, how long each waits for its
//! reply, the fast mode, each request's data size and pattern (`socket.wit` 2.6 `ping-with`, 000-NET-0040), each
//! request's place in the run, and the line `ping` ends with.
use core::fmt::Write;

pub const USAGE: &str = "USAGE: PING [-C COUNT] [-I SECONDS] [-W SECONDS] [-F] [-S BYTES] [-P HEX] HOST";
/// The data bytes `-s` takes (one frame on a 1500-byte MTU), the bytes of a `-p` pattern, and the size `-p` alone sends.
pub const SIZE_MAX: u16 = 1472;
pub const PATTERN_MAX: usize = 16;
pub const PATTERN_SIZE: u16 = 56;
/// Requests without `-c`, and with `-f`.
pub const COUNT: u32 = 3;
pub const FAST_COUNT: u32 = 100;
pub const COUNT_MAX: u32 = 1_000_000;
/// From one request's start to the next one's, and the wait for a reply, without `-i` and `-w` (ms).
pub const INTERVAL_MS: u32 = 1000;
pub const WAIT_MS: u32 = 1000;

/// What `ping` was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Asked<'a> {
    pub host: &'a str,
    /// Requests to send; 0 sends them until Esc or Ctrl+C.
    pub count: u32,
    /// From one request's start to the next one's (ms).
    pub interval_ms: u32,
    /// How long each request waits for its reply (ms); the stack takes 100 ms to 30 s.
    pub wait_ms: u32,
    /// The next request as soon as the last one ended, a dot for each one lost, no line for each reply.
    pub fast: bool,
    /// Each request's data bytes (`-s`; with `-p` alone, 56); None: the stack's own 8 bytes, `MINDCORE`.
    pub size: Option<u16>,
    /// The bytes repeated over the data (`-p`), `pattern_len` of them; none: `MINDCORE`.
    pub pattern: [u8; PATTERN_MAX],
    pub pattern_len: usize,
}

impl Asked<'_> {
    /// Whether the requests carry a size or pattern of their own, through `ping-with`.
    pub fn sized(&self) -> bool { self.size.is_some() }
    pub fn pattern(&self) -> &[u8] { &self.pattern[..self.pattern_len] }
}

pub fn parse(args: &str) -> Result<Asked<'_>, &'static str> {
    let (mut host, mut count, mut interval, mut wait, mut fast) = (None, None, None, WAIT_MS, false);
    let (mut size, mut pattern, mut pattern_len) = (None, [0u8; PATTERN_MAX], 0);
    let mut words = args.split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "-c" | "-C" => count = Some(words.next().and_then(|n| n.parse::<u32>().ok()).filter(|&n| n <= COUNT_MAX).ok_or("PING: -C TAKES 1 TO 1000000 REQUESTS, OR 0: UNTIL ESC")?),
            "-i" | "-I" => interval = Some(words.next().and_then(milliseconds).filter(|&ms| ms <= 60_000).ok_or("PING: -I TAKES 0 TO 60 SECONDS")?),
            "-w" | "-W" => wait = words.next().and_then(milliseconds).filter(|ms| (100..=30_000).contains(ms)).ok_or("PING: -W TAKES 0.1 TO 30 SECONDS")?,
            "-f" | "-F" => fast = true,
            "-s" | "-S" => size = Some(words.next().and_then(|n| n.parse::<u16>().ok()).filter(|&n| n <= SIZE_MAX).ok_or("PING: -S TAKES 0 TO 1472 DATA BYTES")?),
            "-p" | "-P" => pattern_len = words.next().and_then(|hex| bytes(hex, &mut pattern)).ok_or("PING: -P TAKES 1 TO 16 BYTES IN HEXADECIMAL (-P A55A0F)")?,
            _ if word.starts_with('-') || host.is_some() => return Err(USAGE),
            _ => host = Some(word),
        }
    }
    let host = host.ok_or(USAGE)?;
    let count = count.unwrap_or(if fast { FAST_COUNT } else { COUNT });
    let size = size.or((pattern_len > 0).then_some(PATTERN_SIZE));
    Ok(Asked { host, count, interval_ms: interval.unwrap_or(if fast { 0 } else { INTERVAL_MS }), wait_ms: wait, fast, size, pattern, pattern_len })
}

/// Hexadecimal digits in pairs (`a55a0f`, `0xFF`) into `out`; their number of bytes, None if they are not 1 to 16 bytes.
fn bytes(hex: &str, out: &mut [u8; PATTERN_MAX]) -> Option<usize> {
    let hex = hex.strip_prefix("0x").or_else(|| hex.strip_prefix("0X")).unwrap_or(hex).as_bytes();
    if hex.is_empty() || hex.len() % 2 != 0 || hex.len() > 2 * PATTERN_MAX { return None; }
    for (i, pair) in hex.chunks(2).enumerate() {
        out[i] = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(hex.len() / 2)
}

/// Seconds with at most three decimals (`0.2`, `1`, `1.5`) as milliseconds.
fn milliseconds(text: &str) -> Option<u32> {
    let (whole, part) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && part.is_empty() || part.len() > 3 || !whole.bytes().chain(part.bytes()).all(|b| b.is_ascii_digit()) { return None; }
    let whole = if whole.is_empty() { 0 } else { whole.parse::<u32>().ok()? };
    let part = if part.is_empty() { 0 } else { part.parse::<u32>().ok()? * 10u32.pow(3 - part.len() as u32) };
    whole.checked_mul(1000)?.checked_add(part)
}

/// A request's place in the run, `[2/3] `, or `[5] ` for a run until Esc (`-c 0`).
pub fn place(out: &mut impl Write, n: u32, count: u32) {
    let _ = if count == 0 { write!(out, "[{}] ", n) } else { write!(out, "[{}/{}] ", n, count) };
}

/// The requests sent and the replies, with their least, mean and greatest time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally { pub sent: u32, pub received: u32, least: u32, most: u32, total: u64, pub bad: u32 }

impl Tally {
    pub fn reply(&mut self, us: u32) {
        (self.least, self.most) = if self.received == 0 { (us, us) } else { (self.least.min(us), self.most.max(us)) };
        self.sent += 1; self.received += 1; self.total += us as u64;
    }
    pub fn lost(&mut self) { self.sent += 1; }
    /// A reply whose data are not what was sent: received, and counted apart.
    pub fn bad_data(&mut self) { self.bad += 1; }
    /// `PING: 3 SENT, 3 RECEIVED, 0% LOST, TIME 120/340/800 US (LEAST/MEAN/MOST)`, with `, n WITH BAD DATA` when some
    /// replies' data were not what was sent, and `, STOPPED` after Esc.
    pub fn line(&self, out: &mut impl Write, stopped: bool) {
        let lost = if self.sent == 0 { 0 } else { (self.sent - self.received) as u64 * 100 / self.sent as u64 };
        let _ = write!(out, "PING: {} SENT, {} RECEIVED, {}% LOST", self.sent, self.received, lost);
        if self.received > 0 {
            let _ = write!(out, ", TIME {}/{}/{} US (LEAST/MEAN/MOST)", self.least, self.total / self.received as u64, self.most);
        }
        if self.bad > 0 { let _ = write!(out, ", {} WITH BAD DATA", self.bad); }
        let _ = writeln!(out, "{}", if stopped { ", STOPPED" } else { "" });
    }
}
