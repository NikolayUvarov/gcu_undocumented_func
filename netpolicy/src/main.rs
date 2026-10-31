#![no_std]
#![no_main]
// Network policy broker (issue 102, Appendix B.6): grants flow capabilities according to `netpolicy.txt` on the boot
// disk. A grant is a network stack client minted with its own badge; the stack lets its holder reach only what the
// broker registered for that badge (destinations, a term, a volume). Serves idl/netpolicy.wit to launchers; every
// grant, refusal, expiry and revocation goes to the system log, and so does the address a host name of the policy
// resolved to when a grant was made (351-NET-0003). The policy can be changed while the system runs (108): the changed
// policy is kept in the broker's private directory of the boot disk, which only its VFS client opens (`system/
// netpolicy`), and read in place of the shipped file; the shell asks the user before it asks for a change. The policy's
// `default card N` line names the stack's preferred card (550-NET-0025), told to the stack at start, after each change
// and every 10 s (a restarted stack learns it again). A `browse` line gives a program any public address on its ports
// (000-NET-0049); the stack logs each new host it reaches so. A running program may ask, through its grant, for a TCP
// flow its grant does not name (socket.wit 2.8, netpolicy.wit 1.3): the broker names the program and its PID, the shell
// puts the question to the user, and the answer adds the rule (once, for the run, or always as a line of the policy);
// a question not answered within 60 s is refused. Holds: a stack client to mint from (slot 2), a VFS client with the
// broker's own badge (slot 3), the stack's policy client (slot 4).
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, CAP_GRANT, CAP_WRITE};
use mind::idl::codec::{List, Text};
use mind::idl::netpolicy::{self, Error};
use mind::idl::socket::{self, Protocol, Rule};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};
use mind::network::{may_change_policy, BADGE_GRANT_LAST};

const SOURCE: usize = 2; // unbadged stack client: badged children are minted from it
const CONTROL: Endpoint = Endpoint(4); // the stack's policy client (mind::network::BADGE_POLICY)
const POLICY_FILE: &str = "netpolicy.txt"; // the shipped policy
const EDITED_DIR: &str = "system/netpolicy";
const EDITED: &str = "system/netpolicy/netpolicy.txt"; // the policy as changed (108)
const EDITED_NEW: &str = "system/netpolicy/netpolicy.new"; // a change on its way to EDITED
const RECEIVED: usize = 9;
const NO_CARD: u8 = 255; // socket.wit 2.5 `prefer`: no preferred card
const CARDS: u8 = 3; // the stack's cards: two wired, the Wi-Fi station
const PREFER_MS: u64 = 10_000; // how often the preferred card is told to the stack again
const GRANTS: usize = 32;
const SECONDS: u32 = 3600; // a line without a term
const BYTES: u64 = 16 * 1024 * 1024; // a line without a volume
const ANSWER_MS: u64 = 60_000; // a question no one answers in this time is refused (000-NET-0049)

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

// The card the policy prefers as the default (`default card N`, 550-NET-0025): its last such line; NO_CARD without one.
fn default_card(text: &str) -> u8 {
    let mut card = NO_CARD;
    for line in text.lines() {
        let words: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        if let [word, kind, number] = words.as_slice() {
            if word.eq_ignore_ascii_case("default") && kind.eq_ignore_ascii_case("card") {
                if let Some(n) = number.parse::<u8>().ok().filter(|&n| n < CARDS) { card = n; }
            }
        }
    }
    card
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
    let (mut rules, mut seconds, mut bytes) = (Vec::new(), None::<u32>, None::<u64>);
    let Some(text) = policy_text() else { return (rules, 0, 0) };
    let text = text.as_str();
    let resolver = resolver(text);
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        // A browsing grant (000-NET-0049): any public address on its ports, a rule of address 0 each (socket.wit 2.7).
        if let Some((name, ports, count, s, b)) = mind::network::browse_line(line) {
            if !name.eq_ignore_ascii_case(program) { continue; }
            for &port in &ports[..count] { if rules.len() < 16 { rules.push(Rule { address: 0, port, protocol: Protocol::Tcp }); } }
            if let Some(s) = s { seconds = Some(seconds.map_or(s, |t| t.min(s))); }
            if let Some(b) = b { bytes = Some(bytes.map_or(b, |v| v.min(b))); }
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() < 2 || !words[0].eq_ignore_ascii_case(program) || words[0].eq_ignore_ascii_case("resolver") || words[0].eq_ignore_ascii_case("browse") { continue; }
        let rule = if words[1].eq_ignore_ascii_case("dns") {
            (dns != 0).then_some(Rule { address: dns, port: 53, protocol: Protocol::Udp })
        } else {
            let protocol = match words.get(2).map(|w| w.to_ascii_lowercase()) { Some(p) if p == "tcp" => Protocol::Tcp, Some(p) if p == "udp" => Protocol::Udp, Some(p) if p == "icmp" => Protocol::Icmp, _ => continue };
            let address = parse_ip(words[1]).or_else(|| if is_name(words[1]) { lookup(program, words[1], resolver) } else { None });
            address.filter(|&a| a != 0).map(|address| Rule { address, port: words.get(3).and_then(|p| p.parse().ok()).unwrap_or(0), protocol })
        };
        let Some(rule) = rule else { continue };
        if rules.len() < 16 { rules.push(rule); }
        if let Some(s) = words.get(4).and_then(|s| s.parse::<u32>().ok()) { seconds = Some(seconds.map_or(s, |t| t.min(s))); }
        if let Some(b) = words.get(5).and_then(|b| b.parse::<u64>().ok()) { bytes = Some(bytes.map_or(b, |v| v.min(b))); }
    }
    (rules, seconds.unwrap_or(SECONDS), bytes.unwrap_or(BYTES))
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

// Whether `line` is a line the broker understands: a rule, `program dns`, `resolver A.B.C.D[:PORT]` (108),
// `default card N` (550-NET-0025), or `browse program public tcp PORTS [SECONDS [BYTES]]` (000-NET-0049).
fn valid(line: &str) -> bool {
    let words: Vec<&str> = line.split_whitespace().collect();
    if line.contains('#') { return false; }
    if words.first().is_some_and(|w| w.eq_ignore_ascii_case("browse")) { return mind::network::browse_line(line).is_some(); }
    match words.as_slice() {
        [word, kind, number] if word.eq_ignore_ascii_case("default") && kind.eq_ignore_ascii_case("card") => number.parse::<u8>().is_ok_and(|n| n < CARDS),
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

// A program's question (000-NET-0049): the stack's id for it, the grant that asked, what it asks for, since when.
struct Asked { id: u32, badge: u16, pid: u64, program: Text<16>, host: Text<253>, port: u16, why: Text<80>, since: u64 }

// A program's words in the log: printable ASCII only, so its reason cannot forge a line.
fn printable(text: &str) -> String { text.chars().map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '?' }).collect() }

struct Broker { grants: Vec<Grant>, next: u16, preferred: Option<u8>, told: u64, read: bool, asked: Vec<Asked> }

impl Broker {
    fn badge(&mut self) -> u16 {
        // Badges are not reused while a grant with that badge lives.
        loop {
            self.next = if self.next >= BADGE_GRANT_LAST { 1 } else { self.next + 1 };
            if !self.grants.iter().any(|g| g.badge == self.next) { return self.next; }
        }
    }

    // `launcher`: the asking client's badge; a launch-only client (link's) is named in the grant's line (800-NET-0038).
    fn prepare(&mut self, program: &str, launcher: u16) -> Result<u16, Error> {
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
        let browsing = rules.iter().filter(|r| r.address == 0).count();
        mind::println!("[NETPOLICY] GRANT {} TO {}: {} RULES{}, {} S, {} BYTES{}", badge, program, rules.len(),
            if browsing > 0 { ", BROWSING ANY PUBLIC ADDRESS" } else { "" }, seconds, bytes,
            if may_change_policy(launcher) { "" } else { ", ASKED BY A LAUNCH-ONLY CLIENT" });
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

    // The questions the stack holds that are new to the broker, the ones past their time refused, and those whose
    // grant has gone dropped (000-NET-0049).
    fn questions(&mut self, now: u64) {
        let grants = &self.grants;
        self.asked.retain(|q| {
            let live = grants.iter().any(|g| g.badge == q.badge);
            if !live { mind::println!("[NETPOLICY] QUESTION {} OF {}: ITS PROGRAM ENDED", q.id, q.program.as_str()); }
            live
        });
        while let Some(index) = self.asked.iter().position(|q| now - q.since >= ANSWER_MS) {
            let q = self.asked.remove(index);
            let _ = socket::decide(CONTROL, q.id, 0, false, false);
            mind::println!("[NETPOLICY] QUESTION {} OF {}: NO ANSWER IN {} S, REFUSED", q.id, q.program.as_str(), ANSWER_MS / 1000);
        }
        let Ok(Ok(waiting)) = socket::questions(CONTROL) else { return };
        for q in waiting.as_slice() {
            if self.asked.iter().any(|a| a.id == q.id) { continue; }
            let Some(grant) = self.grants.iter().find(|g| g.badge == q.badge) else { let _ = socket::decide(CONTROL, q.id, 0, false, false); continue };
            mind::println!("[NETPOLICY] QUESTION {}: {} (PID {}) ASKS FOR {}:{} TCP: \"{}\"", q.id, grant.program.as_str(), grant.pid,
                printable(q.host.as_str()), q.port, printable(q.why.as_str()));
            self.asked.push(Asked { id: q.id, badge: q.badge, pid: grant.pid, program: grant.program, host: q.host, port: q.port, why: q.why, since: now });
        }
    }

    fn waiting(&self) -> List<netpolicy::Question, 8> {
        let mut list = List::default();
        for q in &self.asked { list.push(netpolicy::Question { id: q.id, pid: q.pid, program: q.program, host: q.host, port: q.port, why: q.why }); }
        list
    }

    // The user's answer to question `id`, through the shell (000-NET-0049); `pid` is the shell's, for the policy's log.
    fn answer(&mut self, id: u32, choice: netpolicy::Choice, pid: u64) -> Result<(), Error> {
        let index = self.asked.iter().position(|q| q.id == id).ok_or(Error::NotFound)?;
        let q = self.asked.remove(index);
        let (program, host) = (q.program.as_str(), q.host.as_str());
        if choice == netpolicy::Choice::No {
            let _ = socket::decide(CONTROL, id, 0, false, false);
            mind::println!("[NETPOLICY] QUESTION {} OF {}: REFUSED BY THE USER", id, program);
            return Ok(());
        }
        let address = parse_ip(host).or_else(|| if is_name(host) { lookup(program, host, resolver(&policy_text().unwrap_or_default())) } else { None });
        let Some(address) = address.filter(|&a| a != 0) else {
            let _ = socket::decide(CONTROL, id, 0, false, false);
            mind::println!("[NETPOLICY] QUESTION {} OF {}: {} NOT RESOLVED, REFUSED", id, program, printable(host));
            return Err(Error::NoNetwork);
        };
        match socket::decide(CONTROL, id, address, true, choice == netpolicy::Choice::Once) {
            Ok(Ok(())) => {}
            Ok(Err(socket::Error::Limit)) => return Err(Error::Limit),
            _ => return Err(Error::NotFound),
        }
        let [a, b, c, d] = address.to_be_bytes();
        let kind = match choice { netpolicy::Choice::Once => "ONCE", netpolicy::Choice::Session => "FOR THIS RUN", _ => "ALWAYS" };
        mind::println!("[NETPOLICY] QUESTION {} OF {}: ALLOWED {} BY THE USER: {}.{}.{}.{}:{} TCP", id, program, kind, a, b, c, d, q.port);
        if choice == netpolicy::Choice::Always { Broker::add(pid, &alloc::format!("{} {} tcp {}", program, host, q.port))?; }
        Ok(())
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

    // Tells the stack the policy's preferred card: read from the policy when `read` or when the last reading failed (the
    // boot disk not mounted yet), else the one known.
    fn prefer(&mut self, read: bool, now: u64) {
        let card = match self.preferred {
            Some(card) if !read && self.read => card,
            _ => { let text = policy_text(); self.read = text.is_some(); default_card(&text.unwrap_or_default()) }
        };
        // Said when the policy names a card, or stops naming one.
        if self.preferred != Some(card) && (card != NO_CARD || self.preferred.is_some()) {
            if card == NO_CARD { mind::println!("[NETPOLICY] NO DEFAULT CARD IN THE POLICY") } else { mind::println!("[NETPOLICY] DEFAULT CARD {} FROM THE POLICY", card) }
        }
        self.preferred = Some(card);
        self.told = now;
        let _ = socket::prefer(CONTROL, card);
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

fn denied(what: &str, pid: u64) -> Error {
    mind::println!("[NETPOLICY] {} REFUSED FOR PID {}: A LAUNCH-ONLY CLIENT", what, pid);
    Error::Denied
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut broker = Broker { grants: Vec::new(), next: 0, preferred: None, told: 0, read: false, asked: Vec::new() };
    mind::println!("[NETPOLICY] READY: POLICY FROM {}", POLICY_FILE);
    broker.prefer(true, mind::time::uptime_ms() as u64);
    let mut checked = 0u64;
    loop {
        let now = mind::time::uptime_ms() as u64;
        if now - checked >= 1000 { checked = now; broker.expire(); broker.questions(now); }
        if now - broker.told >= PREFER_MS { broker.prefer(false, now); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 1000) else { continue };
        if !request.is_call { continue; }
        let _ = match netpolicy::decode(&request, RECEIVED) {
            Err(reason) => wire::reject(reason),
            Ok((netpolicy::Request::Prepare { program }, call)) => netpolicy::reply_prepare(call, broker.prepare(program.as_str(), request.badge)),
            // A launch-only client (link's in its developer mode) may not change the policy or end grants (800-NET-0038).
            Ok((netpolicy::Request::Revoke { .. }, call)) if !may_change_policy(request.badge) => netpolicy::reply_revoke(call, Err(denied("REVOKE", request.sender))),
            Ok((netpolicy::Request::Add { .. }, call)) if !may_change_policy(request.badge) => netpolicy::reply_add(call, Err(denied("ADD", request.sender))),
            Ok((netpolicy::Request::Remove { .. }, call)) if !may_change_policy(request.badge) => netpolicy::reply_remove(call, Err(denied("REMOVE", request.sender))),
            // Only the shell puts a question to the user and answers it (1.3).
            Ok((netpolicy::Request::Questions, call)) if !may_change_policy(request.badge) => netpolicy::reply_questions(call, Err(denied("QUESTIONS", request.sender))),
            Ok((netpolicy::Request::Answer { .. }, call)) if !may_change_policy(request.badge) => netpolicy::reply_answer(call, Err(denied("ANSWER", request.sender))),
            Ok((netpolicy::Request::Questions, call)) => netpolicy::reply_questions(call, Ok(broker.waiting().as_slice())),
            Ok((netpolicy::Request::Answer { id, choice }, call)) => {
                let answered = broker.answer(id, choice, request.sender);
                if answered.is_ok() && choice == netpolicy::Choice::Always { broker.prefer(true, now); }
                netpolicy::reply_answer(call, answered)
            }
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
            Ok((netpolicy::Request::Add { line }, call)) => {
                let added = Broker::add(request.sender, line.as_str());
                if added.is_ok() { broker.prefer(true, now); }
                netpolicy::reply_add(call, added)
            }
            Ok((netpolicy::Request::Remove { line }, call)) => {
                let removed = Broker::remove(request.sender, line.as_str());
                if removed.is_ok_and(|n| n > 0) { broker.prefer(true, now); }
                netpolicy::reply_remove(call, removed)
            }
        };
    }
}
