//! The owner's Effector policy: a bounded text grammar, parsed whole or refused whole; anything not named is denied (750-EFF-0003).

pub const FILE_MAX: usize = 4096;
pub const LINE_MAX: usize = 200;
pub const RULES_MAX: usize = 64;
pub const SERVICES_MAX: usize = 16;
pub const ROOTS_MAX: usize = 8;
pub const KEYS_MAX: usize = 8;
pub const GRANTS_MAX: usize = 8;
pub const NAME_MAX: usize = 32;
pub const PATH_MAX: usize = 128;
pub const BYTES_DEFAULT: u64 = 16 << 20;
pub const BYTES_LIMIT: u64 = 1 << 30;
pub const VERSION: u16 = 1;

/// Services the server may never control: the system's own and the agent's.
pub const PROTECTED_SERVICES: [&str; 4] = ["init", "shell", "effector", "effector_gw"];
/// Trees no transfer may read or write: the agent's files, installed applications, private directories.
pub const PROTECTED_TREES: [&str; 3] = ["data/effector/", "data/apps/", "system/"];
/// Script grants a test may never have: they reach keys, credentials or the firmware.
pub const FORBIDDEN_GRANTS: [&str; 4] = ["firmware", "authority", "tls", "ssh"];
/// The `requires:` words of `msh` (docs/msh.md).
pub const GRANTS: [&str; 18] = ["sysinfo", "file", "files", "log", "lifecycle", "network", "authority", "display", "window-manager", "gpio", "camera", "blockstore", "firmware", "parse", "tls", "battery", "wifi", "ssh"];
pub const LOG_SOURCES: [&str; 2] = ["ring", "boot"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind { TooLarge, NotText, Version, LineTooLong, UnknownKey, Malformed, Duplicate, TooMany, Protected }

/// Where and why a policy was refused; `line` counts from 1, 0 for the whole file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub line: u16, pub kind: ErrorKind }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Start, Stop, Restart }
impl Action { fn bit(self) -> u8 { match self { Self::Start => 1, Self::Stop => 2, Self::Restart => 4 } } }

/// The furthest the updater may go: each step includes the ones before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Update { None, Check, Fetch, Apply }

/// Why a request is denied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny { AgentOff, NotNamed, Protected, Path, TooLarge }

/// A request as the gateway decides it.
#[derive(Clone, Copy, Debug)]
pub enum Ask<'a> {
    Service(&'a str, Action),
    Log(&'a str),
    Collect(&'a str, u64),
    Deliver(&'a str, u64),
    Package(&'a str, u64),
    Update(Update),
    Reboot,
    Screen,
    Test(&'a [&'a str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Text<const N: usize> { bytes: [u8; N], len: u8 }
impl<const N: usize> Text<N> {
    const EMPTY: Self = Self { bytes: [0; N], len: 0 };
    fn new(s: &str) -> Option<Self> {
        if s.is_empty() || s.len() > N || s.len() > u8::MAX as usize { return None; }
        let mut t = Self::EMPTY;
        t.bytes[..s.len()].copy_from_slice(s.as_bytes());
        t.len = s.len() as u8;
        Some(t)
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("") }
}

#[derive(Clone, Copy, Debug)]
pub struct Key { name: Option<Text<NAME_MAX>>, key: [u8; 32] }
impl Key {
    /// The application name the key is trusted for, or `None` for every name (`*`).
    pub fn name(&self) -> Option<&str> { self.name.as_ref().map(|n| n.as_str()) }
    pub fn public(&self) -> &[u8; 32] { &self.key }
}

/// A parsed policy. `Policy::default()` is the empty one: everything denied.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    agent: bool,
    services: [(Text<16>, u8); SERVICES_MAX], services_len: usize,
    logs: u8,
    collect: [Text<PATH_MAX>; ROOTS_MAX], collect_len: usize,
    deliver: Option<Text<PATH_MAX>>,
    packages: bool,
    keys: [Key; KEYS_MAX], keys_len: usize,
    update: Update,
    reboot: bool,
    screen: bool,
    test: Option<([Text<16>; GRANTS_MAX], usize)>,
    max_bytes: u64,
    rules: u16,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            agent: false, services: [(Text::EMPTY, 0); SERVICES_MAX], services_len: 0, logs: 0,
            collect: [Text::EMPTY; ROOTS_MAX], collect_len: 0, deliver: None, packages: false,
            keys: [Key { name: None, key: [0; 32] }; KEYS_MAX], keys_len: 0, update: Update::None,
            reboot: false, screen: false, test: None, max_bytes: BYTES_DEFAULT, rules: 0,
        }
    }
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for (i, pair) in b.chunks(2).enumerate() {
        let d = |c: u8| match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), _ => None };
        out[i] = d(pair[0])? << 4 | d(pair[1])?;
    }
    Some(out)
}

fn name_ok(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// A volume path in the server's form, checked: `data/...` or `log:/...`, no empty, `.` or `..` part, printable
/// ASCII without `\`. Returns it unchanged, so roots and paths compare by prefix.
pub fn path(s: &str) -> Option<&str> {
    if s.is_empty() || s.len() > 255 || !s.bytes().all(|c| (0x21..0x7f).contains(&c) && c != b'\\') { return None; }
    let rest = if s == "log:" || s == "log:/" { "" } else if let Some(r) = s.strip_prefix("log:/") { r } else if s.starts_with("data/") { s } else { return None; };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.is_empty() { return Some(s); }
    if rest.split('/').any(|part| part.is_empty() || part == "." || part == "..") { return None; }
    Some(s)
}

fn root(s: &str) -> Option<Text<PATH_MAX>> {
    let s = if s == "log:" { "log:/" } else { s };
    if !s.ends_with('/') || s.len() > PATH_MAX { return None; }
    Text::new(path(s)?)
}

fn protected(p: &str) -> bool {
    PROTECTED_TREES.iter().any(|t| p.starts_with(t) || (p.len() + 1 == t.len() && t.starts_with(p)))
}

fn under(p: &str, root: &str) -> bool { p.len() > root.len() && p.starts_with(root) }

impl Policy {
    /// Parses a policy file. Any line it does not understand refuses the whole file: a key from a newer version could
    /// narrow what the owner meant, so it is never skipped.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > FILE_MAX { return Err(Error { line: 0, kind: ErrorKind::TooLarge }); }
        let text = core::str::from_utf8(bytes).map_err(|_| Error { line: 0, kind: ErrorKind::NotText })?;
        let mut p = Policy::default();
        let mut seen_version = false;
        let mut seen = [false; 9];
        for (i, raw) in text.split('\n').enumerate() {
            let line_no = (i + 1).min(u16::MAX as usize) as u16;
            let err = |kind| Error { line: line_no, kind };
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.len() > LINE_MAX { return Err(err(ErrorKind::LineTooLong)); }
            if line.bytes().any(|c| c < 0x20 && c != b'\t' || c == 0x7f) { return Err(err(ErrorKind::NotText)); }
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let mut words = line.split_ascii_whitespace();
            let key = words.next().unwrap_or("");
            let args: [&str; 3] = [words.next().unwrap_or(""), words.next().unwrap_or(""), words.next().unwrap_or("")];
            if words.next().is_some() { return Err(err(ErrorKind::Malformed)); }
            if !seen_version {
                if key != "effector-policy" || args[0] != "1" || !args[1].is_empty() { return Err(err(ErrorKind::Version)); }
                seen_version = true;
                continue;
            }
            if p.rules as usize >= RULES_MAX { return Err(err(ErrorKind::TooMany)); }
            p.rules += 1;
            let mut once = |slot: usize| if core::mem::replace(&mut seen[slot], true) { Err(err(ErrorKind::Duplicate)) } else { Ok(()) };
            let switch = |v: &str| match v { "on" => Ok(true), "off" => Ok(false), _ => Err(err(ErrorKind::Malformed)) };
            let only = |n: usize| if args[n..].iter().any(|a| !a.is_empty()) || args[..n].iter().any(|a| a.is_empty()) { Err(err(ErrorKind::Malformed)) } else { Ok(()) };
            match key {
                "agent" => { only(1)?; once(0)?; p.agent = switch(args[0])?; }
                "service" => {
                    only(2)?;
                    if !name_ok(args[0], 16) { return Err(err(ErrorKind::Malformed)); }
                    if PROTECTED_SERVICES.contains(&args[0]) { return Err(err(ErrorKind::Protected)); }
                    let mut bits = 0u8;
                    for a in args[1].split(',') {
                        bits |= match a { "start" => 1, "stop" => 2, "restart" => 4, _ => return Err(err(ErrorKind::Malformed)) };
                    }
                    if p.services[..p.services_len].iter().any(|(n, _)| n.as_str() == args[0]) { return Err(err(ErrorKind::Duplicate)); }
                    if p.services_len == SERVICES_MAX { return Err(err(ErrorKind::TooMany)); }
                    p.services[p.services_len] = (Text::new(args[0]).unwrap(), bits);
                    p.services_len += 1;
                }
                "log" => {
                    only(1)?;
                    let bit = match args[0] { "ring" => 1, "boot" => 2, _ => return Err(err(ErrorKind::Malformed)) };
                    if p.logs & bit != 0 { return Err(err(ErrorKind::Duplicate)); }
                    p.logs |= bit;
                }
                "collect" => {
                    only(1)?;
                    let r = root(args[0]).ok_or(err(ErrorKind::Malformed))?;
                    if protected(r.as_str()) { return Err(err(ErrorKind::Protected)); }
                    if p.collect[..p.collect_len].contains(&r) { return Err(err(ErrorKind::Duplicate)); }
                    if p.collect_len == ROOTS_MAX { return Err(err(ErrorKind::TooMany)); }
                    p.collect[p.collect_len] = r;
                    p.collect_len += 1;
                }
                "deliver" => {
                    only(1)?; once(1)?;
                    let r = root(args[0]).ok_or(err(ErrorKind::Malformed))?;
                    if !r.as_str().starts_with("data/") || r.as_str() == "data/" { return Err(err(ErrorKind::Malformed)); }
                    if protected(r.as_str()) { return Err(err(ErrorKind::Protected)); }
                    p.deliver = Some(r);
                }
                "packages" => { only(1)?; once(2)?; p.packages = switch(args[0])?; }
                "package-key" => {
                    only(2)?;
                    let name = if args[0] == "*" { None } else if name_ok(args[0], NAME_MAX) { Text::new(args[0]) } else { return Err(err(ErrorKind::Malformed)); };
                    let key = hex32(args[1]).ok_or(err(ErrorKind::Malformed))?;
                    if p.keys[..p.keys_len].iter().any(|k| k.name == name && k.key == key) { return Err(err(ErrorKind::Duplicate)); }
                    if p.keys_len == KEYS_MAX { return Err(err(ErrorKind::TooMany)); }
                    p.keys[p.keys_len] = Key { name, key };
                    p.keys_len += 1;
                }
                "update" => {
                    only(1)?; once(3)?;
                    p.update = match args[0] { "off" => Update::None, "check" => Update::Check, "fetch" => Update::Fetch, "apply" => Update::Apply, _ => return Err(err(ErrorKind::Malformed)) };
                }
                "reboot" => { only(1)?; once(4)?; p.reboot = switch(args[0])?; }
                "screen" => { only(1)?; once(5)?; p.screen = switch(args[0])?; }
                "test" => {
                    once(6)?;
                    if !args[1].is_empty() { return Err(err(ErrorKind::Malformed)); }
                    let mut grants = [Text::EMPTY; GRANTS_MAX];
                    let mut n = 0;
                    if args[0] != "none" {
                        for g in args[0].split(',') {
                            if !GRANTS.contains(&g) { return Err(err(ErrorKind::Malformed)); }
                            if FORBIDDEN_GRANTS.contains(&g) { return Err(err(ErrorKind::Protected)); }
                            if grants[..n].iter().any(|t: &Text<16>| t.as_str() == g) { return Err(err(ErrorKind::Duplicate)); }
                            if n == GRANTS_MAX { return Err(err(ErrorKind::TooMany)); }
                            grants[n] = Text::new(g).unwrap();
                            n += 1;
                        }
                    }
                    p.test = Some((grants, n));
                }
                "max-bytes" => {
                    only(1)?; once(7)?;
                    let n: u64 = args[0].parse().map_err(|_| err(ErrorKind::Malformed))?;
                    if n == 0 || n > BYTES_LIMIT || args[0].starts_with('+') || args[0].starts_with('0') { return Err(err(ErrorKind::Malformed)); }
                    p.max_bytes = n;
                }
                _ => return Err(err(ErrorKind::UnknownKey)),
            }
        }
        if !seen_version { return Err(Error { line: 0, kind: ErrorKind::Version }); }
        Ok(p)
    }

    pub fn rules(&self) -> u16 { self.rules }
    /// The root deliveries arrive under (`deliver <root>`), if the policy names one.
    pub fn deliver(&self) -> Option<&str> { self.deliver.as_ref().map(|r| r.as_str()) }
    pub fn agent(&self) -> bool { self.agent }
    pub fn max_bytes(&self) -> u64 { self.max_bytes }
    /// The keys trusted for an application's packages: those named for it and those for every name.
    pub fn keys_for<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a [u8; 32]> + 'a {
        self.keys[..self.keys_len].iter().filter(move |k| k.name().map_or(true, |n| n == name)).map(|k| &k.key)
    }

    /// Decides one request. With `agent off`, or no policy, everything is denied.
    pub fn decide(&self, ask: Ask<'_>) -> Result<(), Deny> {
        if !self.agent { return Err(Deny::AgentOff); }
        let size = |n: u64| if n > self.max_bytes { Err(Deny::TooLarge) } else { Ok(()) };
        match ask {
            Ask::Service(name, action) => {
                if PROTECTED_SERVICES.contains(&name) { return Err(Deny::Protected); }
                match self.services[..self.services_len].iter().find(|(n, _)| n.as_str() == name) {
                    Some((_, bits)) if bits & action.bit() != 0 => Ok(()),
                    _ => Err(Deny::NotNamed),
                }
            }
            Ask::Log(source) => {
                let bit = match source { "ring" => 1, "boot" => 2, _ => return Err(Deny::NotNamed) };
                if self.logs & bit != 0 { Ok(()) } else { Err(Deny::NotNamed) }
            }
            Ask::Collect(p, n) => {
                let p = path(p).ok_or(Deny::Path)?;
                if p.ends_with('/') { return Err(Deny::Path); }
                if protected(p) { return Err(Deny::Protected); }
                if !self.collect[..self.collect_len].iter().any(|r| under(p, r.as_str())) { return Err(Deny::NotNamed); }
                size(n)
            }
            Ask::Deliver(p, n) => {
                let p = path(p).ok_or(Deny::Path)?;
                if p.ends_with('/') { return Err(Deny::Path); }
                if protected(p) { return Err(Deny::Protected); }
                match &self.deliver { Some(r) if under(p, r.as_str()) => size(n), _ => Err(Deny::NotNamed) }
            }
            Ask::Package(name, n) => {
                if !self.packages || !name_ok(name, NAME_MAX) || self.keys_for(name).next().is_none() { return Err(Deny::NotNamed); }
                size(n)
            }
            Ask::Update(step) => if step != Update::None && step <= self.update { Ok(()) } else { Err(Deny::NotNamed) },
            Ask::Reboot => if self.reboot { Ok(()) } else { Err(Deny::NotNamed) },
            Ask::Screen => if self.screen { Ok(()) } else { Err(Deny::NotNamed) },
            Ask::Test(wanted) => {
                let Some((grants, n)) = &self.test else { return Err(Deny::NotNamed) };
                if wanted.iter().all(|w| grants[..*n].iter().any(|g| g.as_str() == *w)) { Ok(()) } else { Err(Deny::NotNamed) }
            }
        }
    }
}
