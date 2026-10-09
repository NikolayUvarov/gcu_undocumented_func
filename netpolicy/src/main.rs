#![no_std]
#![no_main]
// Network policy broker (issue 102, Appendix B.6): grants flow capabilities according to `netpolicy.txt` on the boot
// disk. A grant is a network stack client minted with its own badge; the stack lets its holder reach only what the
// broker registered for that badge (destinations, a term, a volume). Serves idl/netpolicy.wit to launchers; every
// grant, refusal, expiry and revocation goes to the system log, and so does the address a host name of the policy
// resolved to when a grant was made (351-NET-0003). Holds: a stack client to mint from (slot 2), a VFS
// client to read the policy (slot 3), the stack's policy client (slot 4).
extern crate alloc;

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
const POLICY_FILE: &str = "netpolicy.txt";
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
    let Ok(file) = mind::fs::File::open(POLICY_FILE) else { return (rules, 0, 0) };
    let mut text = alloc::vec![0u8; file.size().min(64 * 1024)];
    let len = file.read_at(0, &mut text).unwrap_or(0);
    let text = core::str::from_utf8(&text[..len]).unwrap_or("");
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
        };
    }
}
