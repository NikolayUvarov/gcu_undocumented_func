#![no_std]
#![no_main]
// timed (111-NET-0057): the calendar clock set from time servers, stage 1, SNTP within bounds. init starts it only when
// the owner's policy holds a time line, and gives it the flow grant of program `timed` (its servers, UDP), the rtc
// client that may set the clock, and its private directory, `system/timed`, for the floor. It holds nothing else
// (MC-11.4), and parses the answers itself (MC-11.11).
// At least two servers must agree within a second; a time before the floor (the build's commit time, and the latest
// time it set) is refused, and a step of more than 24 hours is taken only at the first set since boot. Only the
// calendar clock moves: the monotonic clock and every deadline are untouched (MC-5.6).
extern crate alloc;
use alloc::vec::Vec;
use mind::abi::{BootInfo, SLOT_NETWORK};
use mind::idl::{rtc, socket};
use mind::ipc::Endpoint;
use mind::sntp::{self, Choice, Decision, NoChoice, Sample, SERVERS};

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const FLOOR_DIR: &str = "system/timed";
const FLOOR: &str = "system/timed/floor";
const FLOOR_NEW: &str = "system/timed/floor.new";
const ANSWER_MS: usize = 2000; // how long the answers are waited for
const AGAIN_MS: usize = 6 * 3600 * 1000; // after a set
const RETRY_MS: usize = 10_000; // until the first set

// The commit time of the build (MIND_SOURCE_TIME, seconds since 1970, from 02_build.sh): no release runs before it.
const BUILT_MS: i64 = digits(option_env!("MIND_SOURCE_TIME")) * 1000;

const fn digits(text: Option<&str>) -> i64 {
    let Some(text) = text else { return 0 };
    let bytes = text.as_bytes();
    let (mut n, mut i) = (0i64, 0);
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || i >= 12 { return 0; }
        n = n * 10 + (bytes[i] - b'0') as i64;
        i += 1;
    }
    n
}

// The clock as read: its time at `at`, milliseconds of the monotonic clock; later times are counted from there.
#[derive(Clone, Copy)]
struct Base { clock_ms: i64, at: i64 }

impl Base {
    fn now(&self) -> i64 { self.clock_ms + (mind::time::uptime_ms() as i64 - self.at) }
}

// The rtc service's time, taken as its second turns so that it is right to a few milliseconds; none if unreadable.
fn read_clock() -> Option<Base> {
    let read = || -> Option<i64> {
        let date = rtc::date(Endpoint::RTC).ok()??;
        let seconds = rtc::now(Endpoint::RTC).ok()??;
        // A read across midnight: the date again.
        let again = rtc::date(Endpoint::RTC).ok()??;
        (date == again).then(|| sntp::rtc_ms(date, seconds))
    };
    let first = read()?;
    let start = mind::time::uptime_ms();
    while mind::time::uptime_ms() - start < 1100 {
        let now = read()?;
        if now != first { return Some(Base { clock_ms: now, at: mind::time::uptime_ms() as i64 }); }
        mind::time::sleep(5);
    }
    Some(Base { clock_ms: first, at: start as i64 })
}

fn read_floor() -> i64 {
    let Ok(file) = mind::fs::File::open(FLOOR) else { return 0 };
    let mut text = [0u8; 16];
    let n = file.read_at(0, &mut text).unwrap_or(0);
    sntp::parse_floor(&text[..n]).unwrap_or(0)
}

// The floor written whole beside it, then put in its place.
fn write_floor(unix_ms: i64) {
    let mut text = [0u8; 16];
    let n = sntp::floor_text(unix_ms, &mut text);
    let written = match mind::fs::mkdir(FLOOR_DIR) { Ok(()) | Err(mind::fs::Error::Exists) => Ok(()), Err(e) => Err(e) }
        .and_then(|()| mind::fs::File::create(FLOOR_NEW))
        .and_then(|mut file| { file.write_at(0, &text[..n])?; file.flush() })
        .and_then(|()| match mind::fs::remove(FLOOR) { Ok(()) | Err(mind::fs::Error::NotFound) => Ok(()), Err(e) => Err(e) })
        .and_then(|()| mind::fs::rename(FLOOR_NEW, FLOOR));
    if let Err(error) = written { mind::println!("[TIMED] THE FLOOR WAS NOT STORED: {:?}", error); }
}

struct When(i64);
impl core::fmt::Display for When {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let seconds = self.0.div_euclid(1000);
        let (days, second) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
        let (year, month, day) = civil(days);
        write!(f, "{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, second / 3600, second / 60 % 60, second % 60)
    }
}

// The civil date of a day since 1970 (proleptic Gregorian).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (day, month) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    (yoe + era * 400 + (month <= 2) as i64, month, day)
}

struct Server { address: u32, port: u16, nonce: u64, sent: i64, sample: Option<Sample> }

struct Ip(u32, u16);
impl core::fmt::Display for Ip {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "{}.{}.{}.{}:{}", a, b, c, d, self.1)
    }
}

// The time servers the grant names: its UDP rules with an address, each on its port (123 when the rule names none).
fn servers() -> Option<Vec<Server>> {
    let rules = match socket::grant_rules(STACK) {
        Ok(Ok(rules)) => rules,
        other => { mind::println!("[TIMED] ITS GRANT'S RULES NOT READ: {:?}", other.map(|r| r.err())); return None; }
    };
    let list: Vec<Server> = rules.as_slice().iter().filter(|r| r.protocol == socket::Protocol::Udp && r.address != 0).take(SERVERS)
        .map(|r| Server { address: r.address, port: if r.port == 0 { sntp::PORT } else { r.port }, nonce: 0, sent: 0, sample: None }).collect();
    if list.is_empty() { mind::println!("[TIMED] ITS GRANT NAMES NO TIME SERVER (A UDP RULE WITH AN ADDRESS)"); return None; }
    Some(list)
}

fn nonce(salt: u64) -> u64 {
    let n = mind::random::u64().unwrap_or_else(|| mind::time::monotonic_ns().rotate_left(17) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    if n == 0 { 1 } else { n }
}

// Asks every server once, on one socket; the answers come within ANSWER_MS. False without a socket.
fn ask(base: &Base, servers: &mut [Server]) -> bool {
    let socket = match socket::udp_open(STACK, 0) {
        Ok(Ok(socket)) => socket,
        other => { mind::println!("[TIMED] NO UDP SOCKET: {:?}", other.map(|r| r.err())); return false; }
    };
    for (i, server) in servers.iter_mut().enumerate() {
        server.nonce = nonce(i as u64);
        server.sent = base.now();
        if !matches!(socket::udp_send(STACK, socket, server.address, server.port, &sntp::request(server.nonce)), Ok(Ok(()))) {
            mind::println!("[TIMED] {}: THE REQUEST WAS NOT SENT", Ip(server.address, server.port));
        }
    }
    let start = mind::time::uptime_ms();
    while mind::time::uptime_ms() - start < ANSWER_MS && servers.iter().any(|s| s.sample.is_none()) {
        match socket::udp_receive(STACK, socket) {
            Ok(Ok(datagram)) => {
                let came = base.now();
                let Some(server) = servers.iter_mut().find(|s| s.address == datagram.address && s.port == datagram.port && s.sample.is_none()) else { continue };
                match sntp::parse(datagram.data.as_slice(), server.nonce) {
                    Ok(answer) => server.sample = Some(sntp::sample(&answer, server.sent, came)),
                    Err(refused) => mind::println!("[TIMED] {}: ITS ANSWER REFUSED, {}", Ip(server.address, server.port), refused.text()),
                }
            }
            _ => { mind::time::sleep(5); }
        }
    }
    let _ = socket::close(STACK, socket);
    true
}

// One round: the servers asked, the agreeing ones followed within the bounds; the time set, if any.
fn round(floor: i64, first: bool) -> Option<i64> {
    let mut servers = servers()?;
    let Some(base) = read_clock() else { mind::println!("[TIMED] THE CLOCK CANNOT BE READ"); return None };
    if !ask(&base, &mut servers) { return None; }
    for s in servers.iter().filter(|s| s.sample.is_none()) { mind::println!("[TIMED] {}: NO ANSWER IN {} MS", Ip(s.address, s.port), ANSWER_MS); }
    let answered: Vec<(usize, Sample)> = servers.iter().enumerate().filter_map(|(i, s)| s.sample.map(|sample| (i, sample))).collect();
    let samples: Vec<Sample> = answered.iter().map(|&(_, s)| s).collect();
    let Choice { offset_ms, agreed } = match sntp::choose(&samples) {
        Ok(choice) => choice,
        Err(why) => {
            let why = match why { NoChoice::TooFew => "FEWER THAN 2 ANSWERS", NoChoice::NoneAgree => "NO 2 SERVERS AGREE WITHIN A SECOND", NoChoice::Split => "AS MANY SERVERS AGREE ON ANOTHER TIME" };
            mind::println!("[TIMED] NOTHING SET: {} ({} OF {} ANSWERED)", why, answered.len(), servers.len());
            return None;
        }
    };
    for (k, &(i, sample)) in answered.iter().enumerate() {
        let s = &servers[i];
        let verdict = if agreed >> k & 1 == 1 { "AGREES" } else { "LEFT OUT: IT DISAGREES WITH THE OTHERS" };
        mind::println!("[TIMED] {}: OFFSET {:+} MS, ROUND TRIP {} MS, {}", Ip(s.address, s.port), sample.offset_ms, sample.delay_ms, verdict);
    }
    let clock = base.now();
    let new = clock + offset_ms;
    match sntp::decide(clock, new, floor, first, false) {
        Decision::Set => {}
        Decision::BeforeFloor => { mind::println!("[TIMED] NOTHING SET: {} IS BEFORE THE FLOOR {}", When(new), When(floor)); return None; }
        Decision::StepTooLarge => { mind::println!("[TIMED] NOTHING SET: A STEP OF {} S FROM {} WANTS THE USER'S YES", offset_ms / 1000, When(clock)); return None; }
        Decision::OutOfRange => { mind::println!("[TIMED] NOTHING SET: {} IS PAST WHAT THE CLOCK KEEPS", When(new)); return None; }
    }
    // Set as the new time's second turns, so that the clock is right to the millisecond it can keep.
    let wait = 1000 - new.rem_euclid(1000);
    mind::time::sleep(wait as usize);
    let at = base.now() + offset_ms;
    let Some((date, seconds)) = sntp::rtc_parts(at) else { return None };
    match rtc::set(Endpoint::RTC, date, seconds) {
        Ok(Ok(())) => {
            mind::println!("[TIMED] SET {} (WAS {}, {:+} MS), {} OF {} SERVERS AGREED", When(sntp::rtc_ms(date, seconds)), When(at - offset_ms), offset_ms,
                agreed.count_ones(), servers.len());
            Some(sntp::rtc_ms(date, seconds))
        }
        other => { mind::println!("[TIMED] NOTHING SET: THE CLOCK REFUSED, {:?}", other.map(|r| r.err())); None }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut floor = BUILT_MS.max(read_floor());
    mind::println!("[TIMED] STARTED: THE FLOOR IS {}", When(floor));
    let mut first = true;
    loop {
        match round(floor, first) {
            Some(set) => {
                first = false;
                if set > floor { floor = set; write_floor(set); }
                mind::time::sleep(AGAIN_MS);
            }
            None => { mind::time::sleep(if first { RETRY_MS } else { AGAIN_MS }); }
        }
    }
}
