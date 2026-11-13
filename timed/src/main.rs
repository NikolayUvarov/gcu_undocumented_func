#![no_std]
#![no_main]
// timed (111-NET-0057): the calendar clock set from time servers, stage 1, SNTP within bounds. init starts it only when
// the owner's policy holds a time line, and gives it the flow grant of program `timed` (its servers, UDP), the rtc
// client that may set the clock, and its private directory, `system/timed`, for the floor. It holds nothing else
// (MC-11.4), and parses the answers itself (MC-11.11).
// At least two servers must agree within a second; a time before the floor (the build's commit time, and the latest
// time it set) is refused, and a step of more than 24 hours is taken only at the first set since boot. Only the
// calendar clock moves: the monotonic clock and every deadline are untouched (MC-5.6).
// Stage 2, NTS (RFC 8915, 111-NET-0075): an address its grant names over TCP is an NTS server, whose key establishment
// runs through tls (`connect-alpn` ntske/1, the keys from `export`, tls.wit 1.4), its certificate checked for the name
// the policy gave (socket.wit 2.12); its NTP answers count only when they are authentic under its keys and answer this
// request's unique identifier. An address with a UDP rule alone is a plain server, as in stage 1. When every server
// that agreed is NTS's, a step of more than 24 hours is taken after the first set too; the floor holds either way.
extern crate alloc;
mod siv;

use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, SLOT_NETWORK, SLOT_TLS};
use mind::idl::{rtc, socket, tls};
use mind::ipc::Endpoint;
use mind::nts;
use mind::sntp::{self, Choice, Decision, NoChoice, Sample, SERVERS};

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);
const KE_MS: u32 = 5000; // NTS-KE's connection and handshake, then its response
const KE_RETRY_MS: i64 = 10_000; // after a failed NTS-KE, doubled at each failure
const KE_RETRY_MAX_MS: i64 = 6 * 3600 * 1000;
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
    match written {
        Ok(()) => mind::println!("[TIMED] THE FLOOR IS NOW {}, STORED IN {}", When(unix_ms), FLOOR),
        Err(error) => mind::println!("[TIMED] THE FLOOR WAS NOT STORED: {:?}", error),
    }
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

// An NTS server's keys, wiped when it goes.
struct Keys { c2s: [u8; nts::KEY], s2c: [u8; nts::KEY] }
impl Drop for Keys {
    fn drop(&mut self) {
        for byte in self.c2s.iter_mut().chain(self.s2c.iter_mut()) { unsafe { core::ptr::write_volatile(byte, 0) } }
    }
}

// What NTS-KE gave for one server, kept across rounds: its NTP address, keys and unused cookies.
struct Session { ntp: (u32, u16), keys: Keys, cookies: Vec<Vec<u8>> }

// An NTS server its grant names (a TCP rule): where its key establishment runs, the name its certificate is checked
// for, and what the last one gave.
struct Nts { name: String, address: u32, port: u16, session: Option<Session>, failures: u32, next_ke: i64 }

// One server asked in a round: a plain one, or NTS server `nts` with this request's identifier.
struct Server { address: u32, port: u16, nonce: u64, sent: i64, sample: Option<Sample>, nts: Option<usize>, uid: [u8; nts::UID] }

struct Ip(u32, u16);
impl core::fmt::Display for Ip {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "{}.{}.{}.{}:{}", a, b, c, d, self.1)
    }
}

fn dotted(address: u32) -> String {
    let [a, b, c, d] = address.to_be_bytes();
    alloc::format!("{}.{}.{}.{}", a, b, c, d)
}

fn parse_ip(text: &str) -> Option<u32> {
    let mut parts = text.split('.');
    let mut address = 0u32;
    for _ in 0..4 { address = address << 8 | parts.next()?.parse::<u8>().ok()? as u32; }
    parts.next().is_none().then_some(address)
}

// The grant's rules, read again each round: what the user's answers added counts too.
fn rules() -> Option<Vec<socket::Rule>> {
    match socket::grant_rules(STACK) {
        Ok(Ok(rules)) => Some(rules.as_slice().to_vec()),
        other => { mind::println!("[TIMED] ITS GRANT'S RULES NOT READ: {:?}", other.map(|r| r.err())); None }
    }
}

// The NTS servers the grant names: its TCP rules with an address, each on its port (4460 when the rule names none),
// with the host name the policy gave for the address, else the address itself. At most SERVERS in all.
fn nts_servers(rules: &[socket::Rule]) -> Vec<Nts> {
    let names = match socket::grant_names(STACK) { Ok(Ok(names)) => names.as_slice().to_vec(), _ => Vec::new() };
    rules.iter().filter(|r| r.protocol == socket::Protocol::Tcp && r.address != 0).take(SERVERS).map(|r| {
        let name = names.iter().find(|n| n.address == r.address).map(|n| String::from(n.name.as_str())).unwrap_or_else(|| dotted(r.address));
        Nts { name, address: r.address, port: if r.port == 0 { nts::KE_PORT } else { r.port }, session: None, failures: 0, next_ke: 0 }
    }).collect()
}

// The plain time servers: its UDP rules with an address no TCP rule names, each on its port (123 when the rule names
// none). An address with a TCP rule is an NTS server's, and its UDP rules carry that server's NTP.
fn plain_servers(rules: &[socket::Rule], room: usize) -> Vec<Server> {
    let nts = |address: u32| rules.iter().any(|r| r.protocol == socket::Protocol::Tcp && r.address == address);
    rules.iter().filter(|r| r.protocol == socket::Protocol::Udp && r.address != 0 && !nts(r.address)).take(room)
        .map(|r| Server { address: r.address, port: if r.port == 0 { sntp::PORT } else { r.port }, nonce: 0, sent: 0, sample: None, nts: None, uid: [0; nts::UID] })
        .collect()
}

fn random<const N: usize>() -> Option<[u8; N]> {
    let mut out = [0u8; N];
    mind::random::fill(&mut out).then_some(out)
}

// One NTS key establishment with `server` through tls: ntske/1 agreed, the server's certificate valid for its name,
// the response's cookies, the keys from the exporter, and the NTP server it names reachable under the grant.
fn establish(server: &Nts, rules: &[socket::Rule]) -> Result<Session, String> {
    let session = match tls::attach(TLS, SLOT_NETWORK) {
        Ok(Ok(session)) => session,
        Ok(Err(error)) => return Err(alloc::format!("NO TLS SESSION: {:?}", error)),
        Err(error) => return Err(alloc::format!("NO TLS CLIENT: {:?}", error)),
    };
    let result = exchange(server, session, rules);
    let _ = tls::close(TLS, session);
    result
}

fn exchange(server: &Nts, session: u32, rules: &[socket::Rule]) -> Result<Session, String> {
    match tls::connect_alpn(TLS, session, &server.name, server.address, server.port, nts::ALPN, KE_MS) {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => return Err(alloc::format!("TLS REFUSED IT: {:?}", error)),
        Err(error) => return Err(alloc::format!("TLS DID NOT ANSWER: {:?}", error)),
    }
    let request = nts::ke_request();
    if !matches!(tls::send(TLS, session, &request), Ok(Ok(n)) if n as usize == request.len()) { return Err("THE REQUEST WAS NOT SENT".into()); }
    let (mut data, start) = (Vec::new(), mind::time::uptime_ms());
    let mut buffer = [0u8; 4096];
    let ke = loop {
        let length = (nts::RESPONSE_MAX + 8 - data.len()).min(buffer.len()) as u32;
        match tls::receive(TLS, session, length, &mut buffer) {
            Ok(Ok(n)) => data.extend_from_slice(&buffer[..n]),
            Ok(Err(tls::Error::Again)) => { mind::time::sleep(5); }
            Ok(Err(tls::Error::Closed)) => break nts::ke_response(&data),
            other => return Err(alloc::format!("THE RESPONSE WAS NOT READ: {:?}", other.map(|r| r.err()))),
        }
        match nts::ke_response(&data) {
            Err(nts::KeRefused::Incomplete) if (mind::time::uptime_ms() - start) < KE_MS as usize => {}
            done => break done,
        }
    };
    let ke = ke.map_err(|why| match why {
        nts::KeRefused::Error(code) | nts::KeRefused::Warning(code) => alloc::format!("{} {}", why.text(), code),
        nts::KeRefused::Critical(kind) => alloc::format!("{} ({})", why.text(), kind),
        _ => String::from(why.text()),
    })?;
    let mut keys = Keys { c2s: [0; nts::KEY], s2c: [0; nts::KEY] };
    for (key, s2c) in [(&mut keys.c2s, false), (&mut keys.s2c, true)] {
        match tls::export(TLS, session, nts::EXPORTER_LABEL, &nts::context(s2c), nts::KEY as u8, &mut key[..]) {
            Ok(Ok(n)) if n == nts::KEY => {}
            other => return Err(alloc::format!("NO KEYS FROM TLS: {:?}", other.map(|r| r.err()))),
        }
    }
    // The NTP server it names: an IPv4 address, or none for its own; on the port it names, else 123.
    let address = match ke.server.as_deref() {
        None => server.address,
        Some(text) => parse_ip(text).ok_or_else(|| alloc::format!("IT NAMES ITS NTP SERVER {} BY A HOST NAME, WHICH TIMED DOES NOT RESOLVE", text))?,
    };
    let port = ke.port.unwrap_or(sntp::PORT);
    if !rules.iter().any(|r| r.protocol == socket::Protocol::Udp && r.address == address && (r.port == 0 || r.port == port)) {
        return Err(alloc::format!("THE GRANT DOES NOT REACH ITS NTP SERVER {}", Ip(address, port)));
    }
    Ok(Session { ntp: (address, port), keys, cookies: ke.cookies })
}

// The NTS servers ready for this round: each without cookies runs its key establishment first, when its back-off
// allows; one whose key establishment fails waits 10 s, then twice as long at each failure, up to 6 hours.
fn prepare(list: &mut [Nts], rules: &[socket::Rule]) {
    for server in list.iter_mut() {
        if server.session.as_ref().is_some_and(|s| !s.cookies.is_empty()) { continue; }
        let now = mind::time::uptime_ms() as i64;
        if now < server.next_ke { continue; }
        server.session = None;
        match establish(server, rules) {
            Ok(session) => {
                mind::println!("[TIMED] NTS-KE WITH {} ({}): {} COOKIES, NTP AT {}", server.name, Ip(server.address, server.port), session.cookies.len(),
                    Ip(session.ntp.0, session.ntp.1));
                server.session = Some(session);
                server.failures = 0;
            }
            Err(why) => {
                let wait = (KE_RETRY_MS << server.failures.min(16)).min(KE_RETRY_MAX_MS);
                server.failures += 1;
                server.next_ke = now + wait;
                mind::println!("[TIMED] NTS-KE WITH {} ({}) FAILED: {}; AGAIN IN {} S", server.name, Ip(server.address, server.port), why, wait / 1000);
            }
        }
    }
}

fn nonce(salt: u64) -> u64 {
    let n = mind::random::u64().unwrap_or_else(|| mind::time::monotonic_ns().rotate_left(17) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    if n == 0 { 1 } else { n }
}

// Asks every server once, on one socket; the answers come within ANSWER_MS. False without a socket. An NTS server's
// request carries a fresh identifier, one of its cookies and placeholders for more, sealed under its keys; its answer
// counts only if it is authentic and answers that identifier, and its new cookies are kept.
fn ask(base: &Base, servers: &mut [Server], list: &mut [Nts]) -> bool {
    let socket = match socket::udp_open(STACK, 0) {
        Ok(Ok(socket)) => socket,
        other => { mind::println!("[TIMED] NO UDP SOCKET: {:?}", other.map(|r| r.err())); return false; }
    };
    for (i, server) in servers.iter_mut().enumerate() {
        server.nonce = nonce(i as u64);
        let header = sntp::request(server.nonce);
        let packet = match server.nts.and_then(|k| list[k].session.as_mut()) {
            None => header.to_vec(),
            Some(session) => {
                let (Some(uid), Some(seal_nonce), Some(cookie)) = (random::<{ nts::UID }>(), random::<{ nts::NONCE }>(), session.cookies.pop()) else {
                    mind::println!("[TIMED] {}: NO RANDOM BYTES OR COOKIE FOR ITS NTS REQUEST", Ip(server.address, server.port));
                    continue;
                };
                server.uid = uid;
                let key = &session.keys.c2s;
                nts::request(&header, &uid, &cookie, nts::placeholders(session.cookies.len()), &seal_nonce,
                    |ad, n| siv::seal(key, &[ad, n], &[]).try_into().expect("an empty plaintext seals to the tag alone"))
            }
        };
        server.sent = base.now();
        if !matches!(socket::udp_send(STACK, socket, server.address, server.port, &packet), Ok(Ok(()))) {
            mind::println!("[TIMED] {}: THE REQUEST WAS NOT SENT", Ip(server.address, server.port));
        }
    }
    let start = mind::time::uptime_ms();
    while mind::time::uptime_ms() - start < ANSWER_MS && servers.iter().any(|s| s.sample.is_none()) {
        match socket::udp_receive(STACK, socket) {
            Ok(Ok(datagram)) => {
                let came = base.now();
                let Some(server) = servers.iter_mut().find(|s| s.address == datagram.address && s.port == datagram.port && s.sample.is_none()) else { continue };
                let data = datagram.data.as_slice();
                if let Some(k) = server.nts {
                    let Some(session) = list[k].session.as_mut() else { continue };
                    let key = &session.keys.s2c;
                    match nts::answer(data, &server.uid, |ad, n, sealed| siv::open(key, &[ad, n], sealed)) {
                        Ok(cookies) => {
                            for cookie in cookies { if session.cookies.len() < nts::COOKIES { session.cookies.push(cookie); } }
                        }
                        Err(nts::Refused::Nak) => {
                            mind::println!("[TIMED] {}: {}", Ip(server.address, server.port), nts::Refused::Nak.text());
                            list[k].session = None;
                            continue;
                        }
                        Err(refused) => { mind::println!("[TIMED] {}: ITS ANSWER REFUSED, {}", Ip(server.address, server.port), refused.text()); continue; }
                    }
                }
                match sntp::parse(data, server.nonce) {
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

// One round: the servers asked, the agreeing ones followed within the bounds; the time set, if any. `list` holds the
// NTS servers across rounds, as the grant names them.
fn round(floor: i64, first: bool, list: &mut Vec<Nts>) -> Option<i64> {
    let rules = rules()?;
    let named = nts_servers(&rules);
    // The grant's NTS servers; what an earlier round established is kept for those still named.
    let mut kept = core::mem::take(list);
    for server in named {
        let earlier = kept.iter().position(|k| k.address == server.address && k.port == server.port && k.name == server.name);
        list.push(match earlier { Some(i) => kept.swap_remove(i), None => server });
    }
    prepare(list, &rules);
    let mut servers: Vec<Server> = list.iter().enumerate().filter_map(|(k, s)| s.session.as_ref().filter(|s| !s.cookies.is_empty()).map(|session| Server {
        address: session.ntp.0, port: session.ntp.1, nonce: 0, sent: 0, sample: None, nts: Some(k), uid: [0; nts::UID],
    })).collect();
    let room = SERVERS.saturating_sub(list.len());
    servers.extend(plain_servers(&rules, room));
    if servers.is_empty() {
        if list.is_empty() { mind::println!("[TIMED] ITS GRANT NAMES NO TIME SERVER (A UDP OR TCP RULE WITH AN ADDRESS)"); }
        else { mind::println!("[TIMED] NO SERVER TO ASK: NO NTS SERVER HAS KEYS, AND THE GRANT NAMES NO PLAIN ONE"); }
        return None;
    }
    let Some(base) = read_clock() else { mind::println!("[TIMED] THE CLOCK CANNOT BE READ"); return None };
    if !ask(&base, &mut servers, list) { return None; }
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
    let mut by_nts = 0;
    for (k, &(i, sample)) in answered.iter().enumerate() {
        let s = &servers[i];
        let verdict = if agreed >> k & 1 == 1 { "AGREES" } else { "LEFT OUT: IT DISAGREES WITH THE OTHERS" };
        let nts = match s.nts { Some(n) => alloc::format!("AUTHENTICATED BY NTS ({}, {} COOKIES), ", list[n].name, list[n].session.as_ref().map_or(0, |s| s.cookies.len())), None => String::new() };
        if s.nts.is_some() && agreed >> k & 1 == 1 { by_nts += 1; }
        mind::println!("[TIMED] {}: OFFSET {:+} MS, ROUND TRIP {} MS, {}{}", Ip(s.address, s.port), sample.offset_ms, sample.delay_ms, nts, verdict);
    }
    let clock = base.now();
    let new = clock + offset_ms;
    // Every server that agreed authenticated by NTS: a step beyond 24 hours is taken at any set (the floor still holds).
    let authenticated = by_nts == agreed.count_ones();
    match sntp::decide(clock, new, floor, first, authenticated) {
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
            let nts = if by_nts > 0 { alloc::format!(", {} BY NTS", by_nts) } else { String::new() };
            mind::println!("[TIMED] SET {} (WAS {}, {:+} MS), {} OF {} SERVERS AGREED{}", When(sntp::rtc_ms(date, seconds)), When(at - offset_ms), offset_ms,
                agreed.count_ones(), servers.len(), nts);
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
    let mut list = Vec::new();
    loop {
        match round(floor, first, &mut list) {
            Some(set) => {
                first = false;
                if set > floor { floor = set; write_floor(set); }
                mind::time::sleep(AGAIN_MS);
            }
            None => { mind::time::sleep(if first { RETRY_MS } else { AGAIN_MS }); }
        }
    }
}
