//! `net`: diagnostics of the network card driver (idl/net.wit) through the client init grants the shell.
use core::fmt::Write;
use mind::abi::SLOT_NET;
use mind::idl::net;
use mind::ipc::Endpoint;

const SOURCE: [u8; 4] = [10, 0, 2, 15]; // the address QEMU user networking hands out
const ARP_TRIES: usize = 100; // 10 ms apart

fn mac(out: &mut impl Write, mac: &[u8]) { for (i, byte) in mac.iter().enumerate() { let _ = write!(out, "{}{:02X}", if i == 0 { "" } else { ":" }, byte); } }

fn ipv4(text: &str) -> Option<[u8; 4]> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(address)
}

pub fn command(out: &mut impl Write, args: &[u8]) {
    let endpoint = Endpoint(SLOT_NET);
    let info = match net::info(endpoint) {
        Ok(Ok(info)) => info,
        Ok(Err(_)) | Err(_) => { let _ = writeln!(out, "NET: NO NETWORK CARD"); return; }
    };
    let own = info.mac.to_be_bytes(); let own = [own[2], own[3], own[4], own[5], own[6], own[7]];
    let text = core::str::from_utf8(args).unwrap_or("");
    let mut words = text.split_whitespace();
    match (words.next(), words.next()) {
        (None, _) => {
            let counters = net::counters(endpoint).unwrap_or_default();
            let _ = write!(out, "NET MAC="); mac(out, &own);
            let _ = writeln!(out, " LINK={} MTU={} SENT={} RECEIVED={} DROPPED={} INTERRUPTS={}", if info.link { "UP" } else { "DOWN" }, info.mtu,
                             counters.sent, counters.received, counters.dropped, counters.interrupts);
        }
        (Some(w), Some(target)) if w.eq_ignore_ascii_case("arp") => {
            let Some(target) = ipv4(target) else { let _ = writeln!(out, "NET ARP <A.B.C.D>"); return };
            arp(out, endpoint, own, target);
        }
        _ => { let _ = writeln!(out, "NET [ARP <A.B.C.D>]"); }
    }
}

// Broadcasts "who has `target`" and waits up to a second for the answer.
fn arp(out: &mut impl Write, endpoint: Endpoint, own: [u8; 6], target: [u8; 4]) {
    let mut frame = [0u8; 42];
    frame[0..6].fill(0xFF); frame[6..12].copy_from_slice(&own); frame[12..14].copy_from_slice(&[0x08, 0x06]);
    frame[14..22].copy_from_slice(&[0, 1, 0x08, 0, 6, 4, 0, 1]); // Ethernet, IPv4, request
    frame[22..28].copy_from_slice(&own); frame[28..32].copy_from_slice(&SOURCE); frame[38..42].copy_from_slice(&target);
    if !matches!(net::send(endpoint, &frame), Ok(Ok(()))) { let _ = writeln!(out, "NET: SEND FAILED"); return; }
    let mut buffer = [0u8; 1514];
    for _ in 0..ARP_TRIES {
        match net::receive(endpoint, &mut buffer) {
            Ok(Ok(len)) => {
                let reply = &buffer[..len.min(buffer.len())];
                if reply.len() >= 42 && reply[12..14] == [0x08, 0x06] && reply[20..22] == [0, 2] && reply[28..32] == target {
                    let _ = write!(out, "ARP {}.{}.{}.{} IS AT ", target[0], target[1], target[2], target[3]); mac(out, &reply[22..28]);
                    let _ = writeln!(out);
                    return;
                }
            }
            _ => { mind::time::sleep(10); }
        }
    }
    let _ = writeln!(out, "ARP {}.{}.{}.{}: NO ANSWER", target[0], target[1], target[2], target[3]);
}

// Commands over the network stack (idl/socket.wit): ip, ping, nslookup, fetch.
use mind::abi::SLOT_SOCKET;
use mind::idl::socket::{self, Error};

const STACK: Endpoint = Endpoint(SLOT_SOCKET);

fn dotted(out: &mut impl Write, address: u32) { let b = address.to_be_bytes(); let _ = write!(out, "{}.{}.{}.{}", b[0], b[1], b[2], b[3]); }
fn failed(out: &mut impl Write, what: &str, error: Error) { let _ = writeln!(out, "{}: {:?}", what, error); }
fn stack_failed(out: &mut impl Write) { let _ = writeln!(out, "NET: NO NETWORK STACK"); }

// An address from `a.b.c.d` or a name looked up through the configured DNS server.
fn address(out: &mut impl Write, host: &str) -> Option<u32> {
    if let Some(ip) = ipv4(host) { return Some(u32::from_be_bytes(ip)); }
    match socket::resolve(STACK, host, 0, 0, 3000) {
        Ok(Ok(address)) => Some(address),
        Ok(Err(error)) => { failed(out, host, error); None }
        Err(_) => { stack_failed(out); None }
    }
}

pub fn ip(out: &mut impl Write, args: &[u8]) {
    // ip offload on|off (issue 106): the cards complete TCP and UDP checksums of sent frames, or the stack does.
    let mut words = core::str::from_utf8(args).unwrap_or("").split_whitespace();
    if let Some(word) = words.next() {
        if word != "offload" { let _ = writeln!(out, "IP [OFFLOAD ON|OFF]"); return }
        let enable = match words.next().unwrap_or("") { "on" => true, "off" => false, _ => { let _ = writeln!(out, "IP OFFLOAD ON|OFF"); return } };
        match socket::offload(STACK, enable) {
            Ok(Ok(cards)) => { let _ = writeln!(out, "CHECKSUM OFFLOAD {}: {} CARD(S)", if enable { "ON" } else { "OFF" }, cards.count_ones()); }
            Ok(Err(error)) => failed(out, "IP OFFLOAD", error),
            Err(_) => stack_failed(out),
        }
        return;
    }
    match socket::config(STACK) {
        Ok(Ok(c)) => {
            let _ = write!(out, "IP "); dotted(out, c.address); let _ = write!(out, "/{} GATEWAY ", c.prefix); dotted(out, c.gateway);
            let _ = write!(out, " DNS "); dotted(out, c.dns); let _ = writeln!(out, " ({})", if c.dhcp { "DHCP" } else { "STATIC" });
        }
        Ok(Err(error)) => failed(out, "IP", error),
        Err(_) => { stack_failed(out); return }
    }
    // Every card (issue 105): its address and the frames it carried.
    if let Ok(Ok(interfaces)) = socket::interfaces(STACK) {
        for i in interfaces.as_slice() {
            let _ = write!(out, "CARD {} MAC ", i.card); mac(out, &i.mac.to_be_bytes()[2..]);
            let _ = write!(out, " IP "); dotted(out, i.address); let _ = write!(out, "/{} GATEWAY ", i.prefix); dotted(out, i.gateway);
            let _ = writeln!(out, " SENT={} RECEIVED={}", i.sent, i.received);
        }
    }
}

pub fn ping(out: &mut impl Write, args: &[u8]) {
    let Some(host) = core::str::from_utf8(args).ok().and_then(|t| t.split_whitespace().next()) else { let _ = writeln!(out, "PING <HOST>"); return };
    let Some(target) = address(out, host) else { return };
    let mut received = 0;
    for _ in 0..3 {
        match socket::ping(STACK, target, 1000) {
            Ok(Ok(us)) => { received += 1; let _ = write!(out, "REPLY FROM "); dotted(out, target); let _ = writeln!(out, ": TIME={} US", us); }
            Ok(Err(error)) => failed(out, "PING", error),
            Err(_) => { stack_failed(out); return; }
        }
    }
    let _ = writeln!(out, "PING: 3 SENT, {} RECEIVED", received);
}

pub fn nslookup(out: &mut impl Write, args: &[u8]) {
    let text = core::str::from_utf8(args).unwrap_or("");
    let mut words = text.split_whitespace();
    let Some(name) = words.next() else { let _ = writeln!(out, "NSLOOKUP <NAME> [SERVER[:PORT]]"); return };
    let (server, port) = match words.next() {
        None => (0, 0),
        Some(server) => {
            let (host, port) = server.split_once(':').unwrap_or((server, "53"));
            match (ipv4(host), port.parse::<u16>()) { (Some(ip), Ok(port)) => (u32::from_be_bytes(ip), port), _ => { let _ = writeln!(out, "NSLOOKUP: BAD SERVER"); return } }
        }
    };
    match socket::resolve(STACK, name, server, port, 3000) {
        Ok(Ok(address)) => { let _ = write!(out, "NAME {} ADDRESS ", name); dotted(out, address); let _ = writeln!(out); }
        Ok(Err(error)) => failed(out, name, error),
        Err(_) => stack_failed(out),
    }
}

// fetch <host>[:port] [path]: HTTP/1.0 GET; prints the response (up to 2 KiB) and its size.
pub fn fetch(out: &mut impl Write, args: &[u8]) {
    let text = core::str::from_utf8(args).unwrap_or("");
    let mut words = text.split_whitespace();
    let Some(target) = words.next() else { let _ = writeln!(out, "FETCH <HOST>[:PORT] [PATH]"); return };
    let path = words.next().unwrap_or("/");
    let (host, port) = target.split_once(':').unwrap_or((target, "80"));
    let Ok(port) = port.parse::<u16>() else { let _ = writeln!(out, "FETCH: BAD PORT"); return };
    let Some(address) = address(out, host) else { return };
    let handle = match socket::tcp_connect(STACK, address, port, 5000) {
        Ok(Ok(handle)) => handle,
        Ok(Err(error)) => { failed(out, "FETCH", error); return }
        Err(_) => { stack_failed(out); return }
    };
    let mut request = mind::util::FixedBuf::<512>::new();
    let _ = write!(request, "GET {} HTTP/1.0\r\nHost: {}\r\nUser-Agent: mind-core\r\n\r\n", path, host);
    let mut sent = 0;
    while sent < request.as_bytes().len() {
        match socket::tcp_send(STACK, handle, &request.as_bytes()[sent..]) {
            Ok(Ok(n)) => { sent += n as usize; if n == 0 { mind::time::sleep(10); } }
            _ => { let _ = writeln!(out, "FETCH: SEND FAILED"); let _ = socket::close(STACK, handle); return }
        }
    }
    let (mut total, mut shown) = (0usize, 0usize);
    let mut buffer = [0u8; 4096];
    let deadline = mind::time::uptime_ms() + 10_000;
    loop {
        match socket::tcp_receive(STACK, handle, 4096, &mut buffer) {
            Ok(Ok(n)) => {
                let show = n.min(2048 - shown);
                for &byte in &buffer[..show] { let _ = out.write_char(if byte == b'\n' || (0x20..0x7F).contains(&byte) { byte as char } else if byte == b'\r' { continue } else { '.' }); }
                shown += show; total += n;
            }
            Ok(Err(Error::Again)) if mind::time::uptime_ms() < deadline => { mind::time::sleep(10); }
            Ok(Err(Error::Closed)) => break,
            Ok(Err(error)) => { failed(out, "FETCH", error); break }
            Err(_) => { stack_failed(out); break }
        }
    }
    let _ = socket::close(STACK, handle);
    let _ = writeln!(out, "\nFETCH: {} BYTES", total);
}

// Flow grants of the network policy broker (idl/netpolicy.wit, issue 102).
use mind::abi::SLOT_NETPOLICY;
use mind::idl::netpolicy;

const BROKER: Endpoint = Endpoint(SLOT_NETPOLICY);

/// Asks the broker for a grant for program `path` and passes it to `lend` (through the fixed slot `receive`); a
/// program the policy names nothing for runs without the network.
pub fn grant(out: &mut impl Write, path: &str, receive: usize, lend: impl FnOnce(usize) -> Result<(), mind::sys::Error>) -> Result<Option<u16>, mind::sys::Error> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let program = file.strip_suffix(".elf").or_else(|| file.strip_suffix(".ELF")).unwrap_or(file);
    let badge = match netpolicy::prepare(BROKER, program) {
        Ok(Ok(badge)) => badge,
        Ok(Err(error)) => { let _ = writeln!(out, "NETWORK FOR {}: {:?}", program, error); return Ok(None); }
        Err(_) => { let _ = writeln!(out, "NETWORK FOR {}: NO POLICY BROKER", program); return Ok(None); }
    };
    if !matches!(netpolicy::take(BROKER, badge, receive), Ok(Ok(()))) { let _ = writeln!(out, "NETWORK FOR {}: GRANT LOST", program); return Ok(None); }
    let result = lend(receive);
    let _ = mind::ipc::drop_cap(receive); // the loader holds its copy now
    result.map(|()| Some(badge))
}

/// Ties grant `badge` to the started program: the broker drops it when the program ends.
pub fn bind(badge: u16, pid: u64) { let _ = netpolicy::bind(BROKER, badge, pid); }

pub fn grants(out: &mut impl Write) {
    match netpolicy::list(BROKER) {
        Ok(list) if list.is_empty() => { let _ = writeln!(out, "NO NETWORK GRANTS"); }
        Ok(list) => for g in list.as_slice() { let _ = writeln!(out, "GRANT {} {} RULES={} LEFT={} S USED={} BYTES", g.badge, g.program, g.rules, g.left_ms / 1000, g.used); },
        Err(_) => { let _ = writeln!(out, "NET: NO POLICY BROKER"); }
    }
}

pub fn revoke(out: &mut impl Write, args: &[u8]) {
    let Some(program) = core::str::from_utf8(args).ok().and_then(|t| t.split_whitespace().next()) else { let _ = writeln!(out, "NETREVOKE <PROGRAM>"); return };
    match netpolicy::revoke(BROKER, program) {
        Ok(Ok(n)) => { let _ = writeln!(out, "REVOKED {} GRANTS OF {}", n, program); }
        Ok(Err(error)) => { let _ = writeln!(out, "NETREVOKE: {:?}", error); }
        Err(_) => { let _ = writeln!(out, "NET: NO POLICY BROKER"); }
    }
}

// netpolicy [add LINE | remove LINE] (108, netpolicy.wit 1.1): the lines of the policy in force; a change only after
// the user agreed to it on the keyboard or the serial line, which no program can type into.
pub fn policy(shell: &mut crate::Shell, args: &[u8]) {
    let args = core::str::from_utf8(args).unwrap_or("").trim();
    let (verb, line) = args.split_once(char::is_whitespace).map_or((args, ""), |(v, l)| (v, l.trim()));
    let out = &mut shell.term;
    match verb {
        "" => {
            let mut start = 0;
            loop {
                let Ok(lines) = netpolicy::lines(BROKER, start) else { let _ = writeln!(out, "NET: NO POLICY BROKER"); return };
                if lines.is_empty() { break; }
                for line in lines.as_slice() { let _ = writeln!(out, "{}", line); }
                start += lines.len() as u32;
            }
            if start == 0 { let _ = writeln!(out, "NETPOLICY: NO LINES"); }
        }
        "add" | "remove" if !line.is_empty() => {
            let question = alloc::format!("{} THE NETWORK POLICY LINE \"{}\"?", if verb == "add" { "ADD" } else { "REMOVE" }, line);
            if !crate::msh::ask(shell, &question) { let _ = writeln!(shell.term, "NETPOLICY: NOT CHANGED"); return; }
            let out = &mut shell.term;
            let result = if verb == "add" { netpolicy::add(BROKER, line).map(|r| r.map(|()| 1)) } else { netpolicy::remove(BROKER, line) };
            match result {
                Ok(Ok(n)) => { let _ = writeln!(out, "NETPOLICY: {} {} LINE(S)", if verb == "add" { "ADDED" } else { "REMOVED" }, n); }
                Ok(Err(error)) => { let _ = writeln!(out, "NETPOLICY: {:?}", error); }
                Err(_) => { let _ = writeln!(out, "NET: NO POLICY BROKER"); }
            }
        }
        _ => { let _ = writeln!(out, "USAGE: NETPOLICY [ADD <LINE> | REMOVE <LINE>]"); }
    }
}

// TLS over the shell's own flow (idl/tls.wit, issue 103): https, tls cert.
use mind::abi::SLOT_TLS;
use mind::idl::tls;

const TLS: Endpoint = Endpoint(SLOT_TLS);

// https [-c] <host>[:port] [path] [name]: HTTP/1.0 GET over TLS 1.3; the server must present a certificate for `name`
// (default: the host) from a root in tlsroots.pem. -c offers the device certificate when the server asks for one.
pub fn https(out: &mut impl Write, args: &[u8]) {
    let text = core::str::from_utf8(args).unwrap_or("");
    let mut words = text.split_whitespace().peekable();
    let client_certificate = words.next_if_eq(&"-c").is_some();
    let Some(target) = words.next() else { let _ = writeln!(out, "HTTPS [-C] <HOST>[:PORT] [PATH] [NAME]"); return };
    let path = words.next().unwrap_or("/");
    let (host, port) = target.split_once(':').unwrap_or((target, "443"));
    let name = words.next().unwrap_or(host);
    let Ok(port) = port.parse::<u16>() else { let _ = writeln!(out, "HTTPS: BAD PORT"); return };
    let Some(address) = address(out, host) else { return };
    let session = match tls::attach(TLS, SLOT_SOCKET) {
        Ok(Ok(session)) => session,
        Ok(Err(error)) => { let _ = writeln!(out, "HTTPS: {:?}", error); return }
        Err(_) => { let _ = writeln!(out, "HTTPS: NO TLS SERVICE"); return }
    };
    match tls::connect(TLS, session, name, address, port, client_certificate, 10_000) {
        Ok(Ok(peer)) => { let _ = writeln!(out, "HTTPS: {} VERIFIED, TLS 1.3 SUITE {:04X}{}", name, peer.suite, if peer.client_certificate { ", CLIENT CERTIFICATE SENT" } else { "" }); }
        Ok(Err(error)) => { let _ = writeln!(out, "HTTPS: {:?}", error); let _ = tls::close(TLS, session); return }
        Err(_) => { let _ = writeln!(out, "HTTPS: NO TLS SERVICE"); return }
    }
    let mut request = mind::util::FixedBuf::<512>::new();
    let _ = write!(request, "GET {} HTTP/1.0\r\nHost: {}\r\nUser-Agent: mind-core\r\n\r\n", path, name);
    if !matches!(tls::send(TLS, session, request.as_bytes()), Ok(Ok(_))) { let _ = writeln!(out, "HTTPS: SEND FAILED"); let _ = tls::close(TLS, session); return }
    let (mut total, mut shown) = (0usize, 0usize);
    let mut buffer = [0u8; 4096];
    let deadline = mind::time::uptime_ms() + 10_000;
    loop {
        match tls::receive(TLS, session, 4096, &mut buffer) {
            Ok(Ok(n)) => {
                let show = n.min(2048 - shown);
                for &byte in &buffer[..show] { let _ = out.write_char(if byte == b'\n' || (0x20..0x7F).contains(&byte) { byte as char } else if byte == b'\r' { continue } else { '.' }); }
                shown += show; total += n;
            }
            Ok(Err(tls::Error::Again)) if mind::time::uptime_ms() < deadline => { mind::time::sleep(10); }
            Ok(Err(tls::Error::Closed)) => break,
            Ok(Err(error)) => { let _ = writeln!(out, "HTTPS: {:?}", error); break }
            Err(_) => { let _ = writeln!(out, "HTTPS: NO TLS SERVICE"); break }
        }
    }
    let _ = tls::close(TLS, session);
    let _ = writeln!(out, "\nHTTPS: {} BYTES", total);
}

// tls cert: the device certificate in PEM (the key service keeps the private key).
pub fn tls_command(out: &mut impl Write, args: &[u8]) {
    if core::str::from_utf8(args).unwrap_or("").trim() != "cert" { let _ = writeln!(out, "TLS CERT"); return }
    let mut der = [0u8; 512];
    let length = match tls::certificate(TLS, &mut der) {
        Ok(Ok(length)) => length,
        Ok(Err(error)) => { let _ = writeln!(out, "TLS: {:?}", error); return }
        Err(_) => { let _ = writeln!(out, "TLS: NO TLS SERVICE"); return }
    };
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let _ = writeln!(out, "-----BEGIN CERTIFICATE-----");
    for (index, chunk) in der[..length].chunks(3).enumerate() {
        let bits = chunk.iter().enumerate().fold(0u32, |acc, (i, &b)| acc | (b as u32) << (16 - 8 * i));
        for i in 0..4 { let _ = out.write_char(if i <= chunk.len() { ALPHABET[(bits >> (18 - 6 * i) & 63) as usize] as char } else { '=' }); }
        if index % 16 == 15 { let _ = writeln!(out); }
    }
    if length % 48 != 0 { let _ = writeln!(out); }
    let _ = writeln!(out, "-----END CERTIFICATE-----");
}
