// data/services.txt (173-KRN-0035): which boot services init leaves off at boot, and which wait for another. Read once
// the boot volume is there; a bad line is refused and logged, and never stops the boot.

pub const FILE: &str = "data/services.txt";
pub const MAX: usize = 4096;

// What the file may turn off or order: services init starts after vfs_server, the essential ones aside (the core,
// loader, the link, the Effector services, which data/mode.conf decides, the shell, and tpm, which seals the device key).
pub const CONFIGURABLE: [&str; 23] = ["blockstore", "gpio", "battery", "audio_gw", "tts", "video_gw", "virtio_net", "rtl8168", "rtw89_wifi", "rtw88_wifi", "bt_hci", "bcm_wifi",
                                      "keystore", "wlansim", "wlan", "netstack", "netpolicy", "parse", "tls", "ssh", "windows", "sysmon", "updater"];
// What one cannot work without: turning the second off turns the first off, and the first starts after it.
pub const NEEDS: [(&str, &str); 6] = [("netpolicy", "netstack"), ("tls", "netstack"), ("tls", "keystore"), ("ssh", "netstack"), ("ssh", "keystore"), ("tts", "audio_gw")];
// What init grants the first only when the second already runs: the first starts after it.
pub const BEFORE: [(&str, &str); 17] = [("wlan", "bcm_wifi"), ("wlan", "rtw88_wifi"), ("wlan", "wlansim"), ("wlan", "keystore"), ("netstack", "virtio_net"), ("netstack", "rtl8168"),
                                        ("netstack", "wlan"), ("updater", "tls"), ("updater", "parse"), ("updater", "netpolicy"), ("shell", "battery"),
                                        ("shell", "wlan"), ("shell", "ssh"), ("shell", "updater"), ("shell", "blockstore"), ("shell", "gpio"), ("shell", "video_gw")];

// A unit's image name: `virtio_net#1` is an instance of `virtio_net`.
pub fn base(name: &str) -> &str { name.split('#').next().unwrap_or(name) }
pub fn configurable(name: &str) -> bool { CONFIGURABLE.contains(&base(name)) }

// The positions a name stands for: the unit itself, and for an image its instances too.
fn matching<'a, const N: usize>(units: &'a [&'a str; N], name: &'a str) -> impl Iterator<Item = usize> + 'a {
    (0..N).filter(move |&u| units[u] == name || (units[u].starts_with(name) && units[u].as_bytes().get(name.len()) == Some(&b'#')))
}

// The positions unit `u` starts after by init's own rules.
fn deps<'a, const N: usize>(units: &'a [&'a str; N], u: usize) -> impl Iterator<Item = usize> + 'a {
    NEEDS.iter().chain(BEFORE.iter()).filter(move |(who, _)| *who == base(units[u])).flat_map(move |&(_, need)| matching(units, need))
}

// The plan over the units in boot order (`units[p]` the name at position p): off by the file, off as it needs one that
// is (the position of that one), and the position it waits for.
pub struct Plan<const N: usize> { pub off: [bool; N], pub needs: [Option<usize>; N], pub after: [Option<usize>; N] }

impl<const N: usize> Plan<N> {
    pub fn none() -> Self { Self { off: [false; N], needs: [None; N], after: [None; N] } }
    // What position `p` still waits for, once the positions in `tried` were: the one the file names, or one init's rules
    // start first that waits itself.
    pub fn waits_for(&self, units: &[&str; N], p: usize, tried: &[bool; N]) -> Option<usize> {
        self.after[p].filter(|&a| !tried[a]).or_else(|| deps(units, p).find(|&d| d < p && !tried[d]))
    }
    pub fn ready(&self, units: &[&str; N], p: usize, tried: &[bool; N]) -> bool { self.waits_for(units, p, tried).is_none() }
    // How many units the file turns off, how many are off as they need one of those, and how many wait for another.
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.off.iter().filter(|&&o| o).count(), self.needs.iter().flatten().count(), self.after.iter().flatten().count())
    }
}

// The directives of `text` over `units` in boot order; `refused(line, why, text)` for each line left out (line 0: the
// file as a whole).
pub fn parse<const N: usize>(text: &[u8], units: &[&str; N], mut refused: impl FnMut(usize, &'static str, &str)) -> Plan<N> {
    let mut plan = Plan::none();
    let text = if text.len() > MAX { refused(0, "LONGER THAN 4 KiB: THE REST LEFT OUT", ""); &text[..MAX] } else { text };
    for (i, raw) in text.split(|&b| b == b'\n').enumerate() {
        let Some(line) = core::str::from_utf8(raw).ok().filter(|l| l.is_ascii()) else { refused(i + 1, "NOT ASCII", ""); continue };
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut words = line.split_ascii_whitespace();
        let why = match (words.next(), words.next(), words.next(), words.next()) {
            (Some(verb @ ("disable" | "enable")), Some(name), None, None) => known(units, name).err().or_else(|| {
                for p in matching(units, name) { plan.off[p] = verb == "disable"; }
                None
            }),
            (Some("after"), Some(name), Some(other), None) => known(units, name).err().or_else(|| after(&mut plan, units, name, other).err()),
            _ => Some("NOT disable, enable OR after WITH ITS NAMES"),
        };
        if let Some(why) = why { refused(i + 1, why, line); }
    }
    // Off as it needs one that is off, through any chain of NEEDS (it is acyclic, so N rounds settle it).
    for _ in 0..N {
        for p in 0..N {
            if plan.off[p] || plan.needs[p].is_some() { continue; }
            let base = base(units[p]);
            plan.needs[p] = NEEDS.iter().filter(|(who, _)| *who == base).flat_map(|&(_, need)| matching(units, need)).find(|&q| plan.off[q] || plan.needs[q].is_some());
        }
    }
    plan
}

// A name the file may change: a unit init has, and not an essential one.
fn known<const N: usize>(units: &[&str; N], name: &str) -> Result<(), &'static str> {
    if matching(units, name).next().is_none() { return Err("UNKNOWN SERVICE"); }
    if !configurable(name) { return Err("ESSENTIAL: INIT ALWAYS STARTS IT"); }
    Ok(())
}

// `after name other`: every position of `name` waits for `other`'s first, unless `other` already starts after it by
// init's rules or the file's (a later `after` for the same name replaces an earlier one).
fn after<const N: usize>(plan: &mut Plan<N>, units: &[&str; N], name: &str, other: &str) -> Result<(), &'static str> {
    let first = matching(units, other).next().ok_or("UNKNOWN SERVICE")?;
    if base(name) == base(other) { return Err("A SERVICE AFTER ITSELF"); }
    let (mut kept, mut count) = ([(0, None); N], 0);
    for p in matching(units, name) { kept[count] = (p, plan.after[p].take()); count += 1; }
    let refused = if reaches(plan, units, first, name, false) { Some("CONTRADICTS INIT: THE OTHER NEEDS IT FIRST") }
                  else if reaches(plan, units, first, name, true) { Some("A CYCLE OF after") } else { None };
    for &(p, was) in &kept[..count] { plan.after[p] = if refused.is_some() { was } else { Some(first) }; }
    refused.map_or(Ok(()), Err)
}

// Whether position `from` starts after a position of `name`: through init's rules, and with `afters` the file's too.
fn reaches<const N: usize>(plan: &Plan<N>, units: &[&str; N], from: usize, name: &str, afters: bool) -> bool {
    let (mut seen, mut stack, mut depth) = ([false; N], [0usize; N], 1);
    stack[0] = from; seen[from] = true;
    while depth > 0 {
        depth -= 1;
        let p = stack[depth];
        if matching(units, name).any(|q| q == p) { return true; }
        for next in deps(units, p).chain(plan.after[p].filter(|_| afters)) {
            if !seen[next] { seen[next] = true; stack[depth] = next; depth += 1; }
        }
    }
    false
}
