//! `ping`'s command line and its tally (000-APP-0095): how many echo requests, how often, how long each waits for its
//! reply, the fast mode, and the line `ping` ends with. The size and the payload of a request wait for the stack to
//! send them (`socket.wit` sends 8 bytes, `MINDCORE`; issues/requests-NET.md).
use core::fmt::Write;

pub const USAGE: &str = "USAGE: PING [-C COUNT] [-I SECONDS] [-W SECONDS] [-F] HOST";
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
}

pub fn parse(args: &str) -> Result<Asked<'_>, &'static str> {
    let (mut host, mut count, mut interval, mut wait, mut fast) = (None, None, None, WAIT_MS, false);
    let mut words = args.split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "-c" | "-C" => count = Some(words.next().and_then(|n| n.parse::<u32>().ok()).filter(|&n| n <= COUNT_MAX).ok_or("PING: -C TAKES 1 TO 1000000 REQUESTS, OR 0: UNTIL ESC")?),
            "-i" | "-I" => interval = Some(words.next().and_then(milliseconds).filter(|&ms| ms <= 60_000).ok_or("PING: -I TAKES 0 TO 60 SECONDS")?),
            "-w" | "-W" => wait = words.next().and_then(milliseconds).filter(|ms| (100..=30_000).contains(ms)).ok_or("PING: -W TAKES 0.1 TO 30 SECONDS")?,
            "-f" | "-F" => fast = true,
            "-s" | "-S" | "-p" | "-P" => return Err("PING: -S AND -P WAIT FOR THE NETWORK STACK TO SEND THEM; ITS REQUESTS CARRY 8 BYTES"),
            _ if word.starts_with('-') || host.is_some() => return Err(USAGE),
            _ => host = Some(word),
        }
    }
    let host = host.ok_or(USAGE)?;
    let count = count.unwrap_or(if fast { FAST_COUNT } else { COUNT });
    Ok(Asked { host, count, interval_ms: interval.unwrap_or(if fast { 0 } else { INTERVAL_MS }), wait_ms: wait, fast })
}

/// Seconds with at most three decimals (`0.2`, `1`, `1.5`) as milliseconds.
fn milliseconds(text: &str) -> Option<u32> {
    let (whole, part) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && part.is_empty() || part.len() > 3 || !whole.bytes().chain(part.bytes()).all(|b| b.is_ascii_digit()) { return None; }
    let whole = if whole.is_empty() { 0 } else { whole.parse::<u32>().ok()? };
    let part = if part.is_empty() { 0 } else { part.parse::<u32>().ok()? * 10u32.pow(3 - part.len() as u32) };
    whole.checked_mul(1000)?.checked_add(part)
}

/// The requests sent and the replies, with their least, mean and greatest time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally { pub sent: u32, pub received: u32, least: u32, most: u32, total: u64 }

impl Tally {
    pub fn reply(&mut self, us: u32) {
        (self.least, self.most) = if self.received == 0 { (us, us) } else { (self.least.min(us), self.most.max(us)) };
        self.sent += 1; self.received += 1; self.total += us as u64;
    }
    pub fn lost(&mut self) { self.sent += 1; }
    /// `PING: 3 SENT, 3 RECEIVED, 0% LOST, TIME 120/340/800 US (LEAST/MEAN/MOST)`, with `, STOPPED` after Esc.
    pub fn line(&self, out: &mut impl Write, stopped: bool) {
        let lost = if self.sent == 0 { 0 } else { (self.sent - self.received) as u64 * 100 / self.sent as u64 };
        let _ = write!(out, "PING: {} SENT, {} RECEIVED, {}% LOST", self.sent, self.received, lost);
        if self.received > 0 {
            let _ = write!(out, ", TIME {}/{}/{} US (LEAST/MEAN/MOST)", self.least, self.total / self.received as u64, self.most);
        }
        let _ = writeln!(out, "{}", if stopped { ", STOPPED" } else { "" });
    }
}
