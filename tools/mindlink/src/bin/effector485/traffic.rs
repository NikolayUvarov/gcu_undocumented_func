//! The cable's traffic since this daemon started (800-LNK-0018): the bytes sent and received, read from the port's
//! counters with no lock, for the page's feed and, while bytes move, for the daemon's own log.
use crate::json::{obj, s, Value};
use crate::port::{RECEIVED, SENT};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The line's speed in bit/s when it is a serial port, 0 for TCP: at ten bits a byte, what it can carry.
static BAUD: AtomicU32 = AtomicU32::new(0);
static START: OnceLock<Instant> = OnceLock::new();
/// How often the log says what moved, while something does: more than QUIET bytes, so the line's keep-warm POLLs
/// (800-LNK-0020) alone do not fill it.
const LOG_EVERY: Duration = Duration::from_secs(5);
const QUIET: u64 = 512;

pub struct Sample { pub sent: u64, pub received: u64, pub ms: u64 }

pub fn sample() -> Sample {
    let ms = START.get_or_init(Instant::now).elapsed().as_millis() as u64;
    Sample { sent: SENT.load(Ordering::Relaxed), received: RECEIVED.load(Ordering::Relaxed), ms }
}

/// The feed's message: the totals and the daemon's clock in ms, from which the page reckons the speed.
pub fn message() -> Value {
    let t = sample();
    obj(vec![("type", s("traffic")), ("sent", Value::Num(t.sent as f64)), ("received", Value::Num(t.received as f64)),
             ("ms", Value::Num(t.ms as f64)), ("baud", Value::Num(BAUD.load(Ordering::Relaxed) as f64))])
}

/// Bytes as the log shows them: B, KiB, MiB, GiB.
pub fn size(bytes: f64) -> String {
    let (mut value, mut unit) = (bytes, 0);
    while value >= 1024.0 && unit < 3 { value /= 1024.0; unit += 1; }
    let name = ["B", "KiB", "MiB", "GiB"][unit];
    if unit == 0 { format!("{value:.0} {name}") } else { format!("{value:.1} {name}") }
}

/// Starts the clock and the log's line: every LOG_EVERY in which more than QUIET bytes moved, the totals, the speed each
/// way and how busy the line was. `baud` is 0 for a line with no speed of its own (TCP).
pub fn start(baud: u32) {
    BAUD.store(baud, Ordering::Relaxed);
    START.get_or_init(Instant::now);
    std::thread::spawn(|| {
        let mut last = sample();
        loop {
            std::thread::sleep(LOG_EVERY);
            let now = sample();
            let (up, down) = (now.sent - last.sent, now.received - last.received);
            if up + down > QUIET {
                let secs = (now.ms - last.ms).max(1) as f64 / 1000.0;
                let busy = match BAUD.load(Ordering::Relaxed) {
                    0 => String::new(),
                    b => format!("; the line {:.0}% busy", (up + down) as f64 * 10.0 / secs / b as f64 * 100.0),
                };
                eprintln!("effector485: the cable: sent {} ({}/s), received {} ({}/s){busy}",
                          size(now.sent as f64), size(up as f64 / secs), size(now.received as f64), size(down as f64 / secs));
            }
            last = now;
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes() {
        assert_eq!(super::size(0.0), "0 B");
        assert_eq!(super::size(1023.0), "1023 B");
        assert_eq!(super::size(1536.0), "1.5 KiB");
        assert_eq!(super::size(3.0 * 1024.0 * 1024.0), "3.0 MiB");
    }
}
