//! `improve`'s words, without system calls (tests/improve_host.rs): a command line split into words, quoted ones
//! whole; the hub's configuration in the program's own directory; IPv4 addresses and hex.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The words of a command line: spaces part them; `"…"` keeps one word whole, with `\"` and `\\` inside, and `'…'`
/// keeps it as written.
pub fn split(line: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) { chars.next(); }
        let Some(&first) = chars.peek() else { return Ok(words) };
        let mut word = String::new();
        if first == '\'' {
            chars.next();
            loop {
                match chars.next() { None => return Err("a quote is not closed"), Some('\'') => break, Some(c) => word.push(c) }
            }
            if chars.peek().is_some_and(|c| !c.is_whitespace()) { return Err("a quoted word runs into the next one"); }
        } else if first == '"' {
            chars.next();
            loop {
                match chars.next() {
                    None => return Err("a quote is not closed"),
                    Some('"') => break,
                    Some('\\') => match chars.next() { Some(c @ ('"' | '\\')) => word.push(c), Some(c) => { word.push('\\'); word.push(c); } None => return Err("a quote is not closed") },
                    Some(c) => word.push(c),
                }
            }
            if chars.peek().is_some_and(|c| !c.is_whitespace()) { return Err("a quoted word runs into the next one"); }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() { break; }
                word.push(c);
                chars.next();
            }
        }
        words.push(word);
    }
}

pub fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4];
    let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

pub fn ipv4_text(a: u32) -> String { let b = a.to_be_bytes(); format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]) }

pub fn hex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = text.get(i * 2..i * 2 + 2)?;
        if !pair.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) { return None; }
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

/// The hub: its name, as its certificate and the network policy know it; its address, or None to resolve the name;
/// its port; one or two pins, its TLS key's SPKI SHA-256 (the current and the next).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config { pub name: String, pub address: Option<u32>, pub port: u16, pub pins: Vec<[u8; 32]> }

/// `hub.conf`: `hub NAME ADDRESS|dns PORT`, then one or two `pin HEX` lines; `#` starts a comment.
pub fn config(text: &str) -> Result<Config, String> {
    let (mut hub, mut pins) = (None, Vec::new());
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let at = |why: &str| format!("hub.conf, line {}: {why}", n + 1);
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["hub", name, address, port] => hub = Some(hub_line(name, address, port).map_err(|why| at(why))?),
            ["pin", pin] => pins.push(hex32(pin).ok_or_else(|| at("a pin is 64 lower-case hex"))?),
            _ => return Err(at("not `hub NAME ADDRESS|dns PORT` or `pin HEX`")),
        }
    }
    let (name, address, port) = hub.ok_or("hub.conf names no hub: improve setup first")?;
    if pins.is_empty() || pins.len() > 2 { return Err("hub.conf: one or two pins".into()); }
    Ok(Config { name, address, port, pins })
}

/// A hub line's words checked: a name of letters, digits, `-` and `.`; an IPv4 address or `dns`; a port.
pub fn hub_line(name: &str, address: &str, port: &str) -> Result<(String, Option<u32>, u16), &'static str> {
    if name.is_empty() || name.len() > 253 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.') { return Err("a hub's name is letters, digits, - and ."); }
    let address = if address == "dns" { None } else { Some(ipv4(address).ok_or("an address is IPv4, or dns to resolve the name")?) };
    let port = port.parse::<u16>().ok().filter(|p| *p > 0).ok_or("a port is 1 to 65535")?;
    Ok((String::from(name), address, port))
}

pub fn config_text(c: &Config) -> String {
    let mut out = format!("# MIND Hub for improve (100003-APP-0151)\nhub {} {} {}\n", c.name, c.address.map(ipv4_text).unwrap_or_else(|| String::from("dns")), c.port);
    for p in &c.pins {
        out.push_str("pin ");
        for b in p { out.push_str(&format!("{b:02x}")); }
        out.push('\n');
    }
    out
}

/// A refusal's time to its end, in words.
pub fn duration(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", s.div_ceil(60)),
        _ => format!("{} h {} min", s / 3600, (s % 3600) / 60),
    }
}
