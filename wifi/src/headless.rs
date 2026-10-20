//! `wifi join`, `wifi status` and `wifi leave` without a screen (550-APP-0078), for scripts and the hardware tests of
//! 550-ASR-0007. The passphrase comes from a file whose first line names the network, never from the command line, and
//! no line made here holds it. No system calls: tests/wifi_host.rs.
use crate::station::{mac, rate, shown, Link, Refusal, Station};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// How long a join may take before `NO ANSWER`, and how often the link is read meanwhile, in ms.
pub const JOIN_MS: u64 = 60_000;
pub const POLL_MS: u64 = 250;

pub const USAGE: &str = "usage: wifi [--demo] | wifi join <name> [--passphrase-file <file>] [--remember] | wifi status | wifi leave";

/// Exit statuses: joined (or done), refused, no answer in time, no station or a wrong command line.
pub const JOINED: u32 = 0;
pub const REFUSED: u32 = 1;
pub const NO_ANSWER: u32 = 2;
pub const NO_STATION: u32 = 3;

/// What the command line asks for without a screen.
#[derive(Debug, PartialEq, Eq)]
pub enum Command { Join { ssid: Vec<u8>, file: Option<String>, remember: bool }, Status, Leave }

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

/// The command `args` asks for, `--demo` aside; None: the screen.
pub fn parse(args: &str) -> Result<Option<Command>, &'static str> {
    let words: Vec<&str> = words(args).into_iter().filter(|w| *w != "--demo").collect();
    match words.as_slice() {
        [] => Ok(None),
        ["status"] => Ok(Some(Command::Status)),
        ["leave"] => Ok(Some(Command::Leave)),
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

/// Joins `ssid` and waits for the station's answer, `JOIN_MS` at most: the exit status and the line to log. `now`
/// reads a clock in ms; `pause` waits that many ms. A station that does not remember networks is asked again without.
pub fn join(station: &mut dyn Station, ssid: &[u8], passphrase: &str, remember: bool, now: &mut dyn FnMut() -> u64, pause: &mut dyn FnMut(u64)) -> (u32, String) {
    let name = shown(ssid);
    let mut answer = station.join(ssid, passphrase, remember);
    let unkept = remember && answer == Err(Refusal::Unsupported);
    if unkept { answer = station.join(ssid, passphrase, false); }
    match answer {
        Ok(()) => {}
        Err(Refusal::NoStation(why)) => return (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        Err(refusal) => return (REFUSED, format!("[WIFI] REFUSED {}: {}", name, refusal.text())),
    }
    let start = now();
    loop {
        match station.status() {
            Ok(Link::Joined { ssid: joined, bssid, channel, rate: units, .. }) if joined == ssid => {
                let kept = if unkept { ", NOT REMEMBERED: THE STATION KEEPS NO NETWORKS" } else { "" };
                return (JOINED, format!("[WIFI] JOINED {} {} CHANNEL {} {} MB/S{}", name, mac(bssid), channel, rate(units), kept));
            }
            Ok(Link::Failed { ssid: failed, reason }) if failed == ssid => return (REFUSED, format!("[WIFI] REFUSED {}: {}", name, reason.text())),
            Err(Refusal::NoStation(why)) => return (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
            _ => {}
        }
        if now().saturating_sub(start) >= JOIN_MS { return (NO_ANSWER, format!("[WIFI] NO ANSWER {} IN {} S", name, JOIN_MS / 1000)); }
        pause(POLL_MS);
    }
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

/// Leaves the network joined.
pub fn leave(station: &mut dyn Station) -> (u32, String) {
    match station.leave() {
        Ok(()) => (JOINED, String::from("[WIFI] LEFT")),
        Err(Refusal::NoStation(why)) => (NO_STATION, format!("[WIFI] NO STATION: {}", why)),
        Err(refusal) => (REFUSED, format!("[WIFI] NOT LEFT: {}", refusal.text())),
    }
}
