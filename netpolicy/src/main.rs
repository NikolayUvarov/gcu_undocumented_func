#![no_std]
#![no_main]
// Network policy broker (issue 102, Appendix B.6): grants flow capabilities according to `netpolicy.txt` on the boot
// disk. A grant is a network stack client minted with its own badge; the stack lets its holder reach only what the
// broker registered for that badge (destinations, a term, a volume). Serves idl/netpolicy.wit to launchers; every
// grant, refusal, expiry and revocation goes to the system log, and so does the address a host name of the policy
// resolved to when a grant was made (351-NET-0003). The policy can be changed while the system runs (108): the changed
// policy is kept in the broker's private directory of the boot disk, which only its VFS client opens (`system/
// netpolicy`), and read in place of the shipped file; the shell asks the user before it asks for a change. Holds: a stack client to mint from (slot 2), a VFS
// client with the broker's own badge (slot 3), the stack's policy client (slot 4).
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, CAP_GRANT, CAP_WRITE};
use mind::idl::codec::{List, Text};
use mind::idl::netpolicy::{self, Error};
use mind::idl::socket::{self, Protocol, Rule};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};
use mind::network::BADGE_GRANT_LAST;

const SOURCE: usize = 2; // unbadged stack client: badged children are minted from it
const CONTROL: Endpoint = Endpoint(4); // the stack's policy client (mind::network::BADGE_POLICY)
const POLICY_FILE: &str = "netpolicy.txt"; // the shipped policy
const EDITED_DIR: &str = "system/netpolicy";
const EDITED: &str = "system/netpolicy/netpolicy.txt"; // the policy as changed (108)
const EDITED_NEW: &str = "system/netpolicy/netpolicy.new"; // a change on its way to EDITED
const RECEIVED: usize = 9;
const GRANTS: usize = 32;
const SECONDS: u32 = 3600; // a line without a term
const BYTES: u64 = 16 * 1024 * 1024; // a line without a volume

struct Grant { badge: u16, program: Text<16>, cap: usize, rules: u8, taken: bool, pid: u64 }

fn parse_ip(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

// The resolver the policy names for its host names (`resolver A.B.C.D[:PORT]`); (0, 0): the stack's DNS server.
fn resolver(text: &str) -> (u32, u16) {
    for line in text.lines() {
        let words: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        if words.len() == 2 && words[0].eq_ignore_ascii_case("resolver") {
            let (address, port) = words[1].split_once(':').unwrap_or((words[1], "53"));
            if let (Some(address), Ok(port)) = (parse_ip(address), port.parse::<u16>()) { return (address, port); }
        }
    }
    (0, 0)
}

// A host name as DNS has it: dot-separated labels of letters, digits and hyphens, at least one letter.
fn is_name(word: &str) -> bool {
    word.len() <= 253 && word.bytes().any(|b| b.is_ascii_alphabetic())
        && word.split('.').all(|label| !label.is_empty() && label.len() <= 63 && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

// The address of `name` now (351-NET-0003): the grant keeps it, later changes of the name do not follow.
fn lookup(program: &str, name: &str, (server, port): (u32, u16)) -> Option<u32> {
    match socket::resolve(CONTROL, name, server, port, 3000) {
        Ok(Ok(address)) if address != 0 => {
            let [a, b, c, d] = address.to_be_bytes();
            mind::println!("[NETPOLICY] {}: {} IS {}.{}.{}.{}", program, name, a, b, c, d);
            Some(address)
        }
        Ok(Err(error)) => { mind::println!("[NETPOLICY] {}: {} NOT RESOLVED ({:?})", program, name, error); None }
        _ => { mind::println!("[NETPOLICY] {}: {} NOT RESOLVED (NO ANSWER)", program, name); None }
    }
}

// The rules, term and volume the policy names for `program`. Lines: `program address tcp|udp|icmp [port [seconds
// [bytes]]]` or `program dns` (UDP to the configured DNS server, port 53); `#` starts a comment. The address may be a
// host name, looked up when the grant is made at the `resolver` the file names (else the stack's DNS server); a name
// that does not resolve gives no rule. The grant gets the shortest term and the smallest volume any of its lines names
// (the defaults where none does).
fn policy_for(program: &str, dns: u32) -> (Vec<Rule>, u32, u64) {
    let (mut rules, mut seconds, mut bytes) = (Vec::new(), u32::MAX, u64::MAX);
    let Some(text) = policy_text() else { return (rules, 0, 0) };
    let text = text.as_str();
    let resolver = resolver(text);
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() < 2 || !words[0].eq_ignore_ascii_case(program) || words[0].eq_ignore_ascii_case("resolver") { continue; }
        let rule = if words[1].eq_ignore_ascii_case("dns") {
            (dns != 0).then_some(Rule { address: dns, port: 53, protocol: Protocol::Udp })
        } else {
            let protocol = match words.get(2).map(|w| w.to_ascii_lowercase()) { Some(p) if p == "tcp" => Protocol::Tcp, Some(p) if p == "udp" => Protocol::Udp, Some(p) if p == "icmp" => Protocol::Icmp, _ => continue };
            let address = parse_ip(words[1]).or_else(|| if is_name(words[1]) { lookup(program, words[1], resolver) } else { None });
            address.filter(|&a| a != 0).map(|address| Rule { address, port: words.get(3).and_then(|p| p.parse().ok()).unwrap_or(0), protocol })
        };
        let Some(rule) = rule else { continue };
        if rules.len() < 16 { rules.push(rule); }
        if let Some(s) = words.get(4).and_then(|s| s.parse().ok()) { seconds = seconds.min(s); }
        if let Some(b) = words.get(5).and_then(|b| b.parse().ok()) { bytes = bytes.min(b); }
    }
    (rules, if seconds == u32::MAX { SECONDS } else { seconds }, if bytes == u64::MAX { BYTES } else { bytes })
}

// The policy in force (108): the changed one, a change cut short before it replaced it, or the shipped one.
fn policy_text() -> Option<String> {
    [EDITED, EDITED_NEW, POLICY_FILE].iter().find_map(|path| {
        let file = mind::fs::File::open(path).ok()?;
        let mut text = alloc::vec![0u8; file.size().min(64 * 1024)];
        let len = file.read_at(0, &mut text).ok()?;
        text.truncate(len);
        String::from_utf8(text).ok()
    })
}

// A line without its comment and with single spaces.
fn normal(line: &str) -> String {
    line.split('#').next().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ")
}

// Whether `line` is a line the broker understands: a rule, `program dns`, or `resolver A.B.C.D[:PORT]` (108).
fn valid(line: &str) -> bool {
    let words: Vec<&str> = line.split_whitespace().collect();
    if line.contains('#') { return false; }
    match words.as_slice() {
        [word, server] if word.eq_ignore_ascii_case("resolver") => {
            let (address, port) = server.split_once(':').unwrap_or((server, "53"));
            parse_ip(address).is_some_and(|a| a != 0) && port.parse::<u16>().is_ok()
        }
        [program, dns] => dns.eq_ignore_ascii_case("dns") && program.len() <= 16,
        [program, destination, protocol, rest @ ..] if program.len() <= 16 && rest.len() <= 3 => {
            let destination = parse_ip(destination).is_some_and(|a| a != 0) || is_name(destination);
            let protocol = ["tcp", "udp", "icmp"].iter().any(|p| protocol.eq_ignore_ascii_case(p));
            let numbers = rest.first().is_none_or(|p| p.parse::<u16>().is_ok()) && rest.get(1).is_none_or(|s| s.parse::<u32>().is_ok())
                && rest.get(2).is_none_or(|b| b.parse::<u64>().is_ok());
            destination && protocol && numbers
        }
        _ => false,
    }
}

// Stores `text` as the changed policy: written whole beside it, then put in its place.
fn store(text: &str) -> Result<(), Error> {
    let written = mind::fs::mkdir(EDITED_DIR)
        .and_then(|()| mind::fs::File::create(EDITED_NEW))
        .and_then(|mut file| { file.write_at(0, text.as_bytes())?; file.flush() });
    if let Err(error) = written { mind::println!("[NETPOLICY] CHANGE NOT STORED: {:?}", error); return Err(Error::Unwritable); }
    match mind::fs::remove(EDITED) { Ok(()) | Err(mind::fs::Error::NotFound) => {} Err(_) => return Err(Error::Unwritable) }
    mind::fs::rename(EDITED_NEW, EDITED).map_err(|error| { mind::println!("[NETPOLICY] CHANGE NOT STORED: {:?}", error); Error::Unwritable })
}

struct Broker { grants: Vec<Grant>, next: u16 }

impl Broker {
    fn badge(&mut self) -> u16 {
        // Badges are not reused while a grant with that badge lives.
        loop {
            self.next = if self.next >= BADGE_GRANT_LAST { 1 } else { self.next + 1 };
            if !self.grants.iter().any(|g| g.badge == self.next) { return self.next; }
        }
    }

    fn prepare(&mut self, program: &str) -> Result<u16, Error> {
        if program.is_empty() { return Err(Error::Invalid); }
        if self.grants.len() >= GRANTS { return Err(Error::Limit); }
        let dns = match socket::config(CONTROL) { Ok(Ok(config)) => config.dns, _ => 0 };
        let (rules, seconds, bytes) = policy_for(program, dns);
        if rules.is_empty() { mind::println!("[NETPOLICY] REFUSED {}: NO POLICY", program); return Err(Error::NoPolicy); }
        let badge = self.badge();
        let cap = ipc::mint_badged(SOURCE, CAP_WRITE | CAP_GRANT, badge).map_err(|_| Error::Limit)?;
        if !matches!(socket::policy_set(CONTROL, badge, &rules, seconds, bytes), Ok(Ok(()))) {
            let _ = ipc::drop_cap(cap);
            mind::println!("[NETPOLICY] REFUSED {}: THE NETWORK STACK DID NOT TAKE THE GRANT", program);
            return Err(Error::NoNetwork);
        }
        self.grants.push(Grant { badge, program: Text::new(program).unwrap_or_default(), cap, rules: rules.len() as u8, taken: false, pid: 0 });
        mind::println!("[NETPOLICY] GRANT {} TO {}: {} RULES, {} S, {} BYTES", badge, program, rules.len(), seconds, bytes);
        Ok(badge)
    }

    // Ends grant `index`: every copy of its capability is removed and its flows closed.
    fn end(&mut self, index: usize, why: &str) {
        let grant = self.grants.swap_remove(index);
        let removed = ipc::revoke(grant.cap).unwrap_or(0);
        let _ = ipc::drop_cap(grant.cap);
        let closed = match socket::policy_drop(CONTROL, grant.badge) { Ok(Ok(n)) => n, _ => 0 };
        mind::println!("[NETPOLICY] {} {} OF {}: {} COPIES REMOVED, {} SOCKETS CLOSED", why, grant.badge, grant.program, removed, closed);
    }

    fn revoke(&mut self, program: &str) -> u32 {
        let mut count = 0;
        while let Some(index) = self.grants.iter().position(|g| g.program.as_str().eq_ignore_ascii_case(program)) { self.end(index, "REVOKED"); count += 1; }
        count
    }

    // Grants whose term or volume ran out, or whose process ended.
    fn expire(&mut self) {
        let mut index = 0;
        while index < self.grants.len() {
            let pid = self.grants[index].pid;
            if pid != 0 && !mind::process::alive(pid) { self.end(index, "PROCESS ENDED, DROPPED"); continue; }
            match socket::policy_usage(CONTROL, self.grants[index].badge) {
                Ok(Ok(usage)) if usage.left_ms == 0 => self.end(index, "EXPIRED"),
                _ => index += 1,
            }
        }
    }

    // The policy's lines from `start`, comments and blank lines left out (108).
    fn lines(start: u32) -> List<Text<160>, 16> {
        let mut list = List::default();
        let text = policy_text().unwrap_or_default();
        for line in text.lines().map(normal).filter(|l| !l.is_empty()).skip(start as usize).take(16) {
            list.push(Text::new(&line).unwrap_or_default());
        }
        list
    }

    fn add(pid: u64, line: &str) -> Result<(), Error> {
        if !valid(line) { mind::println!("[NETPOLICY] CHANGE REFUSED FOR PID {}: NOT A POLICY LINE: {}", pid, line); return Err(Error::Invalid); }
        let mut text = policy_text().unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') { text.push('\n'); }
        text.push_str(&normal(line));
        text.push('\n');
        store(&text)?;
        mind::println!("[NETPOLICY] POLICY CHANGED BY PID {}: ADDED {}", pid, normal(line));
        Ok(())
    }

    fn remove(pid: u64, line: &str) -> Result<u32, Error> {
        let (target, text) = (normal(line), policy_text().unwrap_or_default());
        if target.is_empty() { return Err(Error::Invalid); }
        let kept: Vec<&str> = text.lines().filter(|l| normal(l) != target).collect();
        let removed = (text.lines().count() - kept.len()) as u32;
        if removed > 0 {
            store(&(kept.join("\n") + "\n"))?;
            mind::println!("[NETPOLICY] POLICY CHANGED BY PID {}: REMOVED {} ({} LINES)", pid, target, removed);
        }
        Ok(removed)
    }

    fn list(&self) -> List<netpolicy::Grant, 32> {
        let mut list = List::default();
        for g in &self.grants {
            let (left, used) = match socket::policy_usage(CONTROL, g.badge) { Ok(Ok(u)) => (u.left_ms, u.sent + u.received), _ => (0, 0) };
            list.push(netpolicy::Grant { badge: g.badge, program: g.program, rules: g.rules, left_ms: left, used });
        }
        list
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut broker = Broker { grants: Vec::new(), next: 0 };
    mind::println!("[NETPOLICY] READY: POLICY FROM {}", POLICY_FILE);
    let mut checked = 0u64;
    loop {
        let now = mind::time::uptime_ms() as u64;
        if now - checked >= 1000 { checked = now; broker.expire(); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 1000) else { continue };
        if !request.is_call { continue; }
        let _ = match netpolicy::decode(&request, RECEIVED) {
            Err(reason) => wire::reject(reason),
            Ok((netpolicy::Request::Prepare { program }, call)) => netpolicy::reply_prepare(call, broker.prepare(program.as_str())),
            Ok((netpolicy::Request::Take { badge }, call)) => {
                // The capability goes out once, to the launcher that prepared it.
                let result = match broker.grants.iter_mut().find(|g| g.badge == badge && !g.taken) { Some(g) => { g.taken = true; Ok(g.cap) } None => Err(Error::NotFound) };
                netpolicy::reply_take(call, result)
            }
            Ok((netpolicy::Request::Bind { badge, pid }, call)) => {
                let result = match broker.grants.iter_mut().find(|g| g.badge == badge && g.pid == 0) { Some(g) => { g.pid = pid; Ok(()) } None => Err(Error::NotFound) };
                netpolicy::reply_bind(call, result)
            }
            Ok((netpolicy::Request::Revoke { program }, call)) => netpolicy::reply_revoke(call, Ok(broker.revoke(program.as_str()))),
            Ok((netpolicy::Request::List, call)) => netpolicy::reply_list(call, broker.list().as_slice()),
            Ok((netpolicy::Request::Lines { start }, call)) => netpolicy::reply_lines(call, Broker::lines(start).as_slice()),
            Ok((netpolicy::Request::Add { line }, call)) => netpolicy::reply_add(call, Broker::add(request.sender, line.as_str())),
            Ok((netpolicy::Request::Remove { line }, call)) => netpolicy::reply_remove(call, Broker::remove(request.sender, line.as_str())),
        };
    }
}
