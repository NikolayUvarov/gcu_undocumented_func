//! SNTPv4 for `timed` (RFC 4330; RFC 5905's client mode; 111-NET-0057): the request, the answer checked, each answer's
//! offset and round trip, the servers that agree, and the bounds a new time keeps. No I/O: times are milliseconds since
//! 1970, UTC, on the clock's own base.

pub const PORT: u16 = 123;
pub const PACKET: usize = 48;
/// The servers `timed` asks at most: the time lines of its policy (111).
pub const SERVERS: usize = 4;
/// Two answers agree when their offsets differ by at most this beyond half of their round trips (111).
pub const AGREE_MS: i64 = 1000;
/// A larger step than this from the clock's time is taken only at the first set after boot, or with the user's yes.
pub const STEP_MS: i64 = 24 * 3600 * 1000;
/// Seconds from 1900, NTP's era 0, to 1970.
const UNIX_FROM_NTP: u64 = 2_208_988_800;
/// Seconds from 1970 to 2000-01-01, the rtc service's day 0.
const UNIX_2000: i64 = 946_684_800;
/// The last day the rtc service keeps: 2099-12-31 (rtc.wit 1.2).
const LAST_DAY: i64 = 36_524;

/// Why an answer was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused { Short, NotAServer, Version(u8), Unsynchronised, Stratum(u8), NotOurs, NoTime }

impl Refused {
    pub fn text(&self) -> &'static str {
        match self {
            Refused::Short => "SHORTER THAN 48 BYTES",
            Refused::NotAServer => "NOT A SERVER'S ANSWER",
            Refused::Version(_) => "NOT NTP VERSION 3 OR 4",
            Refused::Unsynchronised => "THE SERVER SAYS IT IS NOT SYNCHRONISED",
            Refused::Stratum(0) => "STRATUM 0 (A KISS-O'-DEATH)",
            Refused::Stratum(_) => "STRATUM ABOVE 15",
            Refused::NotOurs => "ITS ORIGIN TIME IS NOT OUR REQUEST'S",
            Refused::NoTime => "NO RECEIVE OR TRANSMIT TIME",
        }
    }
}

/// The 64-bit NTP timestamp of `unix_ms` (era 0 until 2036-02-07, era 1 after).
pub fn timestamp(unix_ms: i64) -> u64 {
    let ms = unix_ms.max(0) as u64;
    let seconds = (ms / 1000 + UNIX_FROM_NTP) & 0xFFFF_FFFF;
    // Rounded up, so that `unix_ms` gives the same millisecond back.
    seconds << 32 | (((ms % 1000) << 32) + 999) / 1000
}

/// Milliseconds since 1970 of an NTP timestamp: seconds that would fall before 1970 in era 0 are era 1's.
pub fn unix_ms(timestamp: u64) -> i64 {
    let seconds = timestamp >> 32;
    let seconds = if seconds >= UNIX_FROM_NTP { seconds - UNIX_FROM_NTP } else { seconds + (1 << 32) - UNIX_FROM_NTP };
    (seconds * 1000 + (((timestamp & 0xFFFF_FFFF) * 1000) >> 32)) as i64
}

/// A client's request: version 4, mode 3, and `nonce` as its transmit time, which a server answers as the origin time.
/// RFC 5905 lets the client put any value there; an unpredictable one keeps an off-path forger from matching it.
pub fn request(nonce: u64) -> [u8; PACKET] {
    let mut packet = [0u8; PACKET];
    packet[0] = 4 << 3 | 3;
    packet[40..48].copy_from_slice(&nonce.to_be_bytes());
    packet
}

/// What a server's answer says: its stratum, and when it received the request and sent the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Answer { pub stratum: u8, pub receive: u64, pub transmit: u64 }

/// A server's answer to the request sent with `nonce`, or why it is refused.
pub fn parse(packet: &[u8], nonce: u64) -> Result<Answer, Refused> {
    if packet.len() < PACKET { return Err(Refused::Short); }
    let (leap, version, mode) = (packet[0] >> 6, packet[0] >> 3 & 7, packet[0] & 7);
    if mode != 4 { return Err(Refused::NotAServer); }
    if !(3..=4).contains(&version) { return Err(Refused::Version(version)); }
    if leap == 3 { return Err(Refused::Unsynchronised); }
    let stratum = packet[1];
    if stratum == 0 || stratum > 15 { return Err(Refused::Stratum(stratum)); }
    let word = |at: usize| u64::from_be_bytes([packet[at], packet[at + 1], packet[at + 2], packet[at + 3], packet[at + 4], packet[at + 5], packet[at + 6], packet[at + 7]]);
    if word(24) != nonce { return Err(Refused::NotOurs); }
    let (receive, transmit) = (word(32), word(40));
    if receive == 0 || transmit == 0 { return Err(Refused::NoTime); }
    Ok(Answer { stratum, receive, transmit })
}

/// One server's offset (how far the clock is behind it) and round trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample { pub offset_ms: i64, pub delay_ms: i64 }

/// The sample of `answer` to a request that left at `sent_ms` and was answered at `came_ms`, both on the clock's base.
pub fn sample(answer: &Answer, sent_ms: i64, came_ms: i64) -> Sample {
    let (receive, transmit) = (unix_ms(answer.receive), unix_ms(answer.transmit));
    Sample { offset_ms: ((receive - sent_ms) + (transmit - came_ms)) / 2, delay_ms: ((came_ms - sent_ms) - (transmit - receive)).max(0) }
}

fn agree(a: &Sample, b: &Sample) -> bool { (a.offset_ms - b.offset_ms).abs() <= AGREE_MS + (a.delay_ms + b.delay_ms) / 2 }

/// The offset taken from `samples`: the largest set of at least two that all agree with each other, and its median.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice { pub offset_ms: i64, pub agreed: u8 }

/// Why no offset was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoChoice { TooFew, NoneAgree, Split }

/// The servers that agree (`agreed`, a bit a sample), or none: fewer than two answers, no two that agree, or two
/// different sets of the same largest size, which leaves no majority to follow.
pub fn choose(samples: &[Sample]) -> Result<Choice, NoChoice> {
    let n = samples.len().min(8);
    if n < 2 { return Err(NoChoice::TooFew); }
    let (mut best, mut size, mut split) = (0u8, 0u32, false);
    for set in 1u16..1 << n {
        let set = set as u8;
        let members = set.count_ones();
        if members < 2 || members < size { continue; }
        let all = (0..n).filter(|i| set >> i & 1 == 1).all(|i| (0..n).filter(|j| set >> j & 1 == 1).all(|j| agree(&samples[i], &samples[j])));
        if !all { continue; }
        if members > size { (best, size, split) = (set, members, false); } else { split = true; }
    }
    if size == 0 { return Err(NoChoice::NoneAgree); }
    if split { return Err(NoChoice::Split); }
    let mut offsets = [0i64; 8];
    let mut k = 0;
    for i in (0..n).filter(|i| best >> i & 1 == 1) { offsets[k] = samples[i].offset_ms; k += 1; }
    offsets[..k].sort_unstable();
    let median = if k % 2 == 1 { offsets[k / 2] } else { (offsets[k / 2 - 1] + offsets[k / 2]) / 2 };
    Ok(Choice { offset_ms: median, agreed: best })
}

/// What the bounds say of a new time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision { Set, BeforeFloor, StepTooLarge, OutOfRange }

/// Whether the clock may go from `clock_ms` to `new_ms`: never before `floor_ms`, nor past what the rtc service keeps;
/// a step of more than STEP_MS only at the first set since boot (`first`) or with the user's yes (`confirmed`).
pub fn decide(clock_ms: i64, new_ms: i64, floor_ms: i64, first: bool, confirmed: bool) -> Decision {
    if new_ms < floor_ms { return Decision::BeforeFloor; }
    if rtc_parts(new_ms).is_none() { return Decision::OutOfRange; }
    if (new_ms - clock_ms).abs() > STEP_MS && !first && !confirmed { return Decision::StepTooLarge; }
    Decision::Set
}

/// Milliseconds since 1970 of the rtc service's day (since 2000-01-01) and second of the day.
pub fn rtc_ms(date: u32, seconds: u32) -> i64 { (UNIX_2000 + date as i64 * 86_400 + seconds as i64) * 1000 }

/// The rtc service's day and second of `unix_ms`, rounded to the nearest second; none outside 2000 to 2099.
pub fn rtc_parts(unix_ms: i64) -> Option<(u32, u32)> {
    let seconds = (unix_ms + 500).div_euclid(1000) - UNIX_2000;
    let (date, second) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    (0..=LAST_DAY).contains(&date).then_some((date as u32, second as u32))
}

/// The floor kept in `system/time/floor`: the decimal seconds since 1970 of the latest time set, then a newline.
pub fn parse_floor(text: &[u8]) -> Option<i64> {
    let digits = text.strip_suffix(b"\n").unwrap_or(text);
    if digits.is_empty() || digits.len() > 12 || !digits.iter().all(u8::is_ascii_digit) { return None; }
    Some(digits.iter().fold(0i64, |n, &d| n * 10 + (d - b'0') as i64) * 1000)
}

/// The floor's text for `unix_ms`, into `out`; its length.
pub fn floor_text(unix_ms: i64, out: &mut [u8; 16]) -> usize {
    let mut seconds = unix_ms.max(0) / 1000;
    let mut digits = [0u8; 16];
    let mut n = 0;
    loop { digits[n] = b'0' + (seconds % 10) as u8; n += 1; seconds /= 10; if seconds == 0 { break; } }
    for i in 0..n { out[i] = digits[n - 1 - i]; }
    out[n] = b'\n';
    n + 1
}
