//! `wifi join`, `wifi status`, `wifi leave` and `wifi scan` without a screen (550-APP-0078, 550-APP-0117), for scripts and the hardware tests of
//! 550-ASR-0007; `wifi country`, `countries` and `channels`, the country the station runs under (550-APP-0082). The passphrase comes from a file whose first line names the network, never from the command line, and
//! no line made here holds it. No system calls: tests/wifi_host.rs.
use crate::station::{mac, rate, shown, CountryState, CountryStatus, Link, Reason, Refusal, Station};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// How long a join may take before `NO ANSWER`, and how often the link is read meanwhile, in ms.
pub const JOIN_MS: u64 = 60_000;
pub const POLL_MS: u64 = 250;

pub const USAGE: &str = "usage: wifi [--demo] [--countries] | wifi join <name> [--passphrase-file <file>] [--remember] | wifi status | wifi leave | wifi scan\n\
                         | wifi country [<code>] | wifi countries [<text>] | wifi channels [<code>]";

/// The most pages of 32 countries `countries` reads: the catalogue has about 250.
const PAGES: u16 = 16;

/// Exit statuses: joined (or done), refused, no answer in time, no station or a wrong command line.
pub const JOINED: u32 = 0;
pub const REFUSED: u32 = 1;
pub const NO_ANSWER: u32 = 2;
pub const NO_STATION: u32 = 3;

/// What the command line asks for without a screen.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Join { ssid: Vec<u8>, file: Option<String>, remember: bool }, Status, Leave,
    /// The networks around as text, strongest first (550-APP-0117).
    Scan,
    /// The country, or a change to `code` (two letters, either case).
    Country(Option<String>),
    /// The catalogue, its codes and names that hold `text` (any case).
    Countries(Option<String>),
    /// The channels under `code`, or under the country applied.
    Channels(Option<String>),
}

// Two letters, as the catalogue writes them.
fn code(word: &str) -> Option<String> { (word.len() == 2 && word.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| word.to_ascii_uppercase()) }

/// The words of `args`; a word in double quotes may hold spaces (a network's name).
fn words(args: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = args.trim_start();
    while !rest.is_empty() {
        let (word, next) = match rest.strip_prefix('"') {
            Some(quoted) => match quoted.find('"') { Some(end) => (&quoted[..end], &quoted[end + 1..]), None => (quoted, "") },
            None => { let end = rest.find(char::is_whitespace).unwrap_or(rest.len()); (&rest[..end], &rest[end..]) }
        };
        out.push(word);
        rest = next.trim_start();
    }
    out
}

/// The command `args` asks for, `--demo` aside; None: the screen, at its country list with `--countries` alone.
pub fn parse(args: &str) -> Result<Option<Command>, &'static str> {
    let words: Vec<&str> = words(args).into_iter().filter(|w| *w != "--demo").collect();
    match words.as_slice() {
        [] | ["--countries"] => Ok(None),
        ["status"] => Ok(Some(Command::Status)),
        ["leave"] => Ok(Some(Command::Leave)),
        ["scan"] => Ok(Some(Command::Scan)),
        ["country"] => Ok(Some(Command::Country(None))),
        ["country", word] => code(word).map(|c| Some(Command::Country(Some(c)))).ok_or(USAGE),
        ["countries"] => Ok(Some(Command::Countries(None))),
        ["countries", text] => Ok(Some(Command::Countries(Some(text.to_ascii_lowercase())))),
        ["channels"] => Ok(Some(Command::Channels(None))),
        ["channels", word] => code(word).map(|c| Some(Command::Channels(Some(c)))).ok_or(USAGE),
        ["join", name, options @ ..] if !name.starts_with("--") && !name.is_empty() => {
            let (mut file, mut remember, mut rest) = (None, false, options);
            while let [option, more @ ..] = rest {
                match (*option, more) {
                    ("--remember", _) => { remember = true; rest = more; }
                    ("--passphrase-file", [path, after @ ..]) if file.is_none() => { file = Some(String::from(*path)); rest = after; }
                    _ => return Err(USAGE),
                }
            }
            Ok(Some(Command::Join { ssid: Vec::from(name.as_bytes()), file, remember }))
        }
        _ => Err(USAGE),
    }
}

/// The passphrase in a file's `text`: its first line must be `ssid`, its second the passphrase (a CR before a line's end
/// is dropped). The errors never hold the file's lines.
pub fn passphrase(text: &[u8], ssid: &[u8]) -> Result<String, &'static str> {
    let mut lines = text.split(|&b| b == b'\n').map(|line| line.strip_suffix(b"\r").unwrap_or(line));
    if lines.next() != Some(ssid) { return Err("the file's first line is not the network's name"); }
    let secret = lines.next().unwrap_or(b"");
    if secret.is_empty() { return Err("the file has no passphrase on its second line"); }
    core::str::from_utf8(secret).map(String::from).map_err(|_| "the file's passphrase is not text")
}

/// Joins `ssid` and waits for the station's answer, `JOIN_MS` at most: the exit status and the lines to log. `now`
/// reads a clock in ms; `pause` waits that many ms. A station that does not remember networks is asked again without.
/// One whose last scan did not hear `ssid` (a join at start, before any scan) scans once and is asked again
/// (550-APP-0098).
pub fn join(station: &mut dyn Station, ssid: &[u8], passphrase: &str, remember: bool, now: &mut dyn FnMut() -> u64, pause: &mut dyn FnMut(u64)) -> (u32, String) {
    let name = shown(ssid);
    let (mut answer, unkept) = start(station, ssid, passphrase, remember);
    let mut scanned = None;
    if answer == Err(Refusal::NotFound) {
        scanned = Some(scan(station, ssid));
        answer = start(station, ssid, passphrase, remember).0;
    }
    if let Err(refusal) = answer { return with(&scanned, refused(&name, refusal)); }
    let mut begun = now();
    loop {
        match station.status() {
            Ok(Link::Joined { ssid: joined, bssid, channel, rate: units, .. }) if joined == ssid => {
                let kept = if unkept { ", NOT REMEMBERED: THE STATION KEEPS NO NETWORKS" } else { "" };
                return with(&scanned, (JOINED, format!("[WIFI] JOINED {} {} CHANNEL {} {} MB/S{}", name, mac(bssid), channel, rate(units), kept)));
            }
            Ok(Link::Failed { ssid: failed, reason: Reason::NotFound }) if failed == ssid && scanned.is_none() => {
                scanned = Some(scan(station, ssid));
                if let Err(refusal) = start(station, ssid, passphrase, remember).0 { return with(&scanned, refused(&name, refusal)); }
                begun = now();
                continue;
            }
            Ok(Link::Failed { ssid: failed, reason }) if failed == ssid => return with(&scanned, (REFUSED, format!("[WIFI] REFUSED {}: {}", name, reason.text()))),
            Err(Refusal::NoStation(why)) => return with(&scanned, (NO_STATION, format!("[WIFI] NO STATION: {}", why))),
            _ => {}
        }
        if now().saturating_sub(begun) >= JOIN_MS { return with(&scanned, (NO_ANSWER, format!("[WIFI] NO ANSWER {} IN {} S", name, JOIN_MS / 1000))); }
        pause(POLL_MS);
    }
}

// Asks the station to join, and again without `remember` when it keeps no networks (true then).
fn start(station: &mut dyn Station, ssid: &[u8], passphrase: &str, remember: bool) -> (Result<(), Refusal>, bool) {
    let answer = station.join(ssid, passphrase, remember);
    if remember && answer == Err(Refusal::Unsupported) { (station.join(ssid, passphrase, false), true) } else { (answer, false) }
}

// The one scan of a join the last scan did not hear: the line it logs.
fn scan(station: &mut dyn Station, ssid: &[u8]) -> String {
    match station.scan() {
        Ok(heard) => format!("[WIFI] THE LAST SCAN DID NOT HEAR {}: SCANNED, {} ACCESS POINTS, {}", shown(ssid), heard.len(),
                             if heard.iter().any(|n| n.ssid == ssid) { "IT AMONG THEM" } else { "NOT IT" }),
        Err(refusal) => format!("[WIFI] THE LAST SCAN DID NOT HEAR {}: THE SCAN FAILED: {}", shown(ssid), refusal.text()),
    }
}

fn refused(name: &str, refusal: Refusal) -> (u32, String) {
    match refusal {
        Refusal::NoStation(why) => (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        // The window's text names its key to scan again; here a scan already ran.
        Refusal::NotFound => (REFUSED, format!("[WIFI] REFUSED {}: {}", name, Reason::NotFound.text())),
        refusal => (REFUSED, format!("[WIFI] REFUSED {}: {}", name, refusal.text())),
    }
}

// The scan's line, if one ran, before the end's.
fn with(scanned: &Option<String>, (code, line): (u32, String)) -> (u32, String) {
    (code, match scanned { Some(scan) => format!("{}\n{}", scan, line), None => line })
}

/// The link's state as one line, and the exit status.
pub fn status(station: &mut dyn Station) -> (u32, String) {
    let link = match station.status() {
        Ok(link) => link,
        Err(Refusal::NoStation(why)) => return (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        Err(refusal) => return (REFUSED, format!("[WIFI] STATUS UNKNOWN: {}", refusal.text())),
    };
    (JOINED, match link {
        Link::Off => String::from("[WIFI] STATUS OFF"),
        Link::Scanning => String::from("[WIFI] STATUS SCANNING"),
        Link::Joining { ssid } => format!("[WIFI] STATUS JOINING {}", shown(&ssid)),
        Link::Joined { ssid, bssid, channel, weakness, rate: units } => {
            format!("[WIFI] STATUS JOINED {} {} CHANNEL {} {} MB/S {} DBM", shown(&ssid), mac(bssid), channel, rate(units), if weakness == 0 { String::from("?") } else { format!("-{}", weakness) })
        }
        Link::Lost { ssid } => format!("[WIFI] STATUS LOST {}: JOINING IT AGAIN", shown(&ssid)),
        Link::Failed { ssid, reason } => format!("[WIFI] STATUS FAILED {}: {}", shown(&ssid), reason.text()),
    })
}

/// The networks around, one line each, strongest first, then their count (550-APP-0117, for the drivers' hardware
/// evidence): the name as `wifi join` takes it (quoted, `(hidden)` when it has none), the BSSID, the channel, the level,
/// the protection, whether the station remembers it, and the one joined now. 0 even when the list is empty.
pub fn networks(station: &mut dyn Station) -> (u32, String) {
    let mut heard = match station.scan() {
        Ok(heard) => heard,
        Err(Refusal::NoStation(why)) => return (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        Err(refusal) => return (REFUSED, format!("[WIFI] SCAN REFUSED: {}", refusal.text())),
    };
    let joined = match station.status() { Ok(Link::Joined { bssid, .. }) => Some(bssid), _ => None };
    // Strongest first; a level the station did not measure (0) last.
    heard.sort_by_key(|n| (n.weakness == 0, n.weakness));
    let mut out = String::new();
    for n in &heard {
        let name = if n.ssid.is_empty() { String::from("(hidden)") } else { format!("\"{}\"", shown(&n.ssid)) };
        let level = if n.weakness == 0 { String::from("? DBM") } else { format!("-{} DBM", n.weakness) };
        out.push_str(&format!("[WIFI] NETWORK {} {} CHANNEL {} {} {}{}{}\n", name, mac(n.bssid), n.channel, level, n.security.name().to_uppercase(),
                              if n.remembered { " REMEMBERED" } else { "" }, if joined == Some(n.bssid) { " JOINED" } else { "" }));
    }
    out.push_str(&format!("[WIFI] SCAN: {} NETWORK{}", heard.len(), if heard.len() == 1 { "" } else { "S" }));
    (JOINED, out)
}

/// Leaves the network joined.
pub fn leave(station: &mut dyn Station) -> (u32, String) {
    match station.leave() {
        Ok(()) => (JOINED, String::from("[WIFI] LEFT")),
        Err(Refusal::NoStation(why)) => (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        Err(refusal) => (REFUSED, format!("[WIFI] NOT LEFT: {}", refusal.text())),
    }
}

// A country's code with its name: `UZ (Uzbekistan)`, `00 (the world domain)`.
fn named(code: &str) -> String {
    match code {
        "" => String::from("none"),
        "00" => String::from("00 (the world domain)"),
        _ => match crate::countries::name(code) { Some(name) => format!("{} ({})", code, name), None => String::from(code) },
    }
}

// Why a country call was not answered, as a line.
fn country_refused(what: &str, refusal: Refusal) -> (u32, String) {
    match refusal {
        Refusal::NoStation(why) => (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        Refusal::Unsupported => (REFUSED, format!("[WIFI] {}: NO CATALOGUE IN THIS STATION (IT RUNS AS WIFI.WIT 1.0 DID)", what)),
        Refusal::Denied => (REFUSED, format!("[WIFI] {}: WIFI'S CLIENT OF THE STATION ONLY READS: THE COUNTRY NEEDS THE CHANGE BADGE", what)),
        Refusal::NotFound => (REFUSED, format!("[WIFI] {}: NOT A COUNTRY OF THE CATALOGUE", what)),
        refusal => (REFUSED, format!("[WIFI] {}: {}", what, refusal.text())),
    }
}

/// The country's state as one line: asked for and applied apart, whether the radio confirmed its limits and `keystore`
/// keeps it, the catalogue.
pub fn country_line(status: &CountryStatus) -> String {
    if status.catalogue.is_empty() { return String::from("[WIFI] COUNTRY: NO CATALOGUE IN THIS STATION (IT RUNS AS WIFI.WIT 1.0 DID)"); }
    let state = match status.state {
        CountryState::None => String::from("NONE ASKED: THE STATION PROBES NOWHERE AND JOINS NOTHING UNTIL ONE IS (WIFI COUNTRY <CODE>)"),
        CountryState::Applied if status.asked == status.applied => format!("{} APPLIED", named(&status.applied)),
        CountryState::Applied => format!("{} APPLIED, {} ASKED", named(&status.applied), named(&status.asked)),
        CountryState::Failed => format!("{} ASKED, FAILED: THE RADIO REFUSED ITS LIMITS, NONE APPLIED", named(&status.asked)),
    };
    let radio = if status.state != CountryState::Applied { "" } else if status.radio { ", THE RADIO CONFIRMED ITS LIMITS" } else { ", THE STATION ALONE KEEPS TO THE CHANNELS" };
    let kept = if status.state == CountryState::None { "" } else if status.kept { ", KEPT" } else { ", NOT KEPT ACROSS RESTARTS" };
    format!("[WIFI] COUNTRY {}{}{}; CATALOGUE {}", state, radio, kept, status.catalogue)
}

/// `wifi country [<code>]`: the country, or a change to `code` (the station leaves the network first).
pub fn country(station: &mut dyn Station, code: Option<&str>) -> (u32, String) {
    match code {
        None => match station.country() { Ok(status) => (JOINED, country_line(&status)), Err(refusal) => country_refused("COUNTRY", refusal) },
        Some(code) => match station.set_country(code) {
            Ok(status) if status.state == CountryState::Applied => (JOINED, country_line(&status)),
            Ok(status) => (REFUSED, country_line(&status)),
            Err(refusal) => country_refused(&format!("COUNTRY {} NOT SET", code), refusal),
        },
    }
}

/// `wifi countries [<text>]`: every country of the catalogue, a line each, with how many channels it lets the station
/// send on and only listen on; `text` keeps those whose code or name holds it.
pub fn countries(station: &mut dyn Station, text: Option<&str>) -> (u32, String) {
    let (mut lines, mut from, mut total) = (Vec::new(), 0u16, 0u16);
    for _ in 0..PAGES {
        let (all, page) = match station.countries(from) { Ok(answer) => answer, Err(refusal) => return country_refused("COUNTRIES", refusal) };
        total = all;
        for c in &page {
            let name = crate::countries::name(&c.code).unwrap_or("");
            if text.is_some_and(|t| !c.code.to_ascii_lowercase().contains(t) && !name.to_ascii_lowercase().contains(t)) { continue; }
            let note = if c.send == 0 && c.listen == 0 { ": NO CHANNEL OF THIS RADIO" } else { "" };
            lines.push(format!("{} {:<34} SEND {:>2} LISTEN {:>2}{}", c.code, name, c.send, c.listen, note));
        }
        from += page.len() as u16;
        if page.is_empty() || from >= all { break; }
    }
    let shown = lines.len();
    lines.push(format!("[WIFI] {} OF {} COUNTRIES{}", shown, total, text.map_or(String::new(), |t| format!(" HOLD \"{}\"", t))));
    (JOINED, lines.join("\n"))
}

/// `wifi channels [<code>]`: the radio's channels under `code`, or under the country applied, with their standing and
/// the most power allowed.
pub fn channels(station: &mut dyn Station, code: Option<&str>) -> (u32, String) {
    let code = match code {
        Some(code) => String::from(code),
        None => match station.country() {
            Ok(status) if status.state == CountryState::Applied => status.applied,
            Ok(status) if status.catalogue.is_empty() => return country_refused("CHANNELS", Refusal::Unsupported),
            Ok(_) => String::from("00"),
            Err(refusal) => return country_refused("CHANNELS", refusal),
        },
    };
    let list = match station.channels(&code) { Ok(list) => list, Err(refusal) => return country_refused(&format!("CHANNELS OF {}", code), refusal) };
    let mut lines: Vec<String> = list.iter().map(|c| {
        let band = if c.number <= 14 { "2.4 GHZ" } else { "5 GHZ" };
        if c.eirp == 0 { format!("CHANNEL {:>3} {} {}", c.number, band, c.standing.name()) } else { format!("CHANNEL {:>3} {} {} {} DBM", c.number, band, c.standing.name(), c.eirp) }
    }).collect();
    lines.push(format!("[WIFI] {} CHANNELS UNDER {}", list.len(), named(&code)));
    (JOINED, lines.join("\n"))
}
