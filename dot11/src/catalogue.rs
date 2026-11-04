//! The station's country from the regulatory catalogue (550-NET-0035, stage B): the rules it runs under and why. A
//! country the user chose comes first; with none, the country the access points announce; with none announced, `00`,
//! the rules every country allows (the maintainer's decision of 2026-11-04: everything must work, not go quiet).
use crate::country::{self as rules, Rule, Standing};
use crate::station::Allowed;

/// The most channels a radio reports (the station's own bound).
pub const CHANNELS: usize = 64;

/// Why the station runs under its country.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Chosen by the user (`set-country`), kept by `keystore`.
    Chosen,
    /// Announced by `announced` of the `heard` access points that named a country.
    Detected { announced: u8, heard: u8 },
    /// None chosen and none announced: `00`.
    World,
}

/// The country to run under: the code and why. `chosen` is the user's (None: none kept, or not in the catalogue), and
/// `announced` what the access points heard name (`scan::List::announced`).
pub fn decide(chosen: Option<[u8; 2]>, announced: Option<([u8; 2], usize, usize)>) -> ([u8; 2], Source) {
    if let Some(code) = chosen.filter(|&c| regdb::select(c).is_ok()) { return (code, Source::Chosen); }
    match announced {
        Some((code, n, of)) if regdb::select(code).is_ok() => (code, Source::Detected { announced: n.min(255) as u8, heard: of.min(255) as u8 }),
        _ => (*b"00", Source::World),
    }
}

/// Whether a scan's announcement moves the station from the country it runs under: only one the user did not choose,
/// only to another code, and never back to `00`, so that the same rules are not applied again (which leaves the network).
pub fn moves(running: ([u8; 2], Source), announced: Option<([u8; 2], usize, usize)>) -> Option<([u8; 2], Source)> {
    if running.1 == Source::Chosen { return None; }
    let next = decide(None, announced);
    (next.0 != running.0 && next.1 != Source::World).then_some(next)
}

/// The catalogue's entry for `code`, `00` included.
pub fn entry(code: [u8; 2]) -> Option<&'static regdb::Country> {
    if code == *b"00" { Some(regdb::world()) } else { regdb::select(code).ok().map(|s| s.country()) }
}

/// A catalogue rule as the station reads it.
fn rule(r: &regdb::Rule) -> Rule {
    use regdb::Flags;
    let f = r.flags;
    Rule {
        start_khz: r.start_khz, end_khz: r.end_khz, bandwidth_khz: r.max_bandwidth_khz, eirp_mbm: r.max_eirp_mbm,
        no_ir: f.contains(Flags::NO_IR), dfs: f.contains(Flags::DFS),
        one_modulation: f.contains(Flags::NO_OFDM) || f.contains(Flags::NO_CCK),
        one_environment: f.contains(Flags::NO_INDOOR) || f.contains(Flags::NO_OUTDOOR),
        point_to_point: f.contains(Flags::PTP_ONLY) || f.contains(Flags::PTMP_ONLY),
    }
}

/// Up to 32 rules of a country, as the station reads them.
pub fn station_rules(country: &regdb::Country) -> ([Rule; 32], usize) {
    let mut out = [Rule::default(); 32];
    let mut n = 0;
    for (r, _) in country.source_rules().take(32) { out[n] = rule(r); n += 1; }
    (out, n)
}

/// What a country lets the station do on the radio's channels: each channel's allowance, and the limits the radio is
/// given for the ones it may use (the channel, its most EIRP in dBm, and whether it may only listen there).
#[derive(Clone, Copy)]
pub struct Plan {
    pub allowed: [(u8, Allowed); CHANNELS],
    pub count: usize,
    pub limits: [(u8, u8, bool); CHANNELS],
    pub limit_count: usize,
}

impl Plan {
    pub fn allowed(&self) -> &[(u8, Allowed)] { &self.allowed[..self.count] }
    pub fn limits(&self) -> &[(u8, u8, bool)] { &self.limits[..self.limit_count] }
    /// How many channels the station may send on, and only listen on.
    pub fn counts(&self) -> (u8, u8) {
        let n = |want| self.allowed().iter().filter(|(_, a)| *a == want).count().min(255) as u8;
        (n(Allowed::Send), n(Allowed::Listen))
    }
}

/// The plan of `country` over the radio's channels `radio`.
pub fn plan(country: &regdb::Country, radio: &[u8]) -> Plan {
    let (rules_, n) = station_rules(country);
    let mut p = Plan { allowed: [(0, Allowed::Nothing); CHANNELS], count: 0, limits: [(0, 0, false); CHANNELS], limit_count: 0 };
    for &channel in radio.iter().take(CHANNELS) {
        let (standing, eirp) = rules::standing(&rules_[..n], channel, true);
        let a = match standing { Standing::Send => Allowed::Send, Standing::Listen => Allowed::Listen, Standing::Barred | Standing::Radio => Allowed::Nothing };
        p.allowed[p.count] = (channel, a);
        p.count += 1;
        if a != Allowed::Nothing { p.limits[p.limit_count] = (channel, eirp, a == Allowed::Listen); p.limit_count += 1; }
    }
    p
}

/// Before any country is applied: only listening, on every channel, so the first scan sends nothing.
pub fn listening(radio: &[u8]) -> Plan {
    let mut p = Plan { allowed: [(0, Allowed::Nothing); CHANNELS], count: 0, limits: [(0, 0, false); CHANNELS], limit_count: 0 };
    for &channel in radio.iter().take(CHANNELS) { p.allowed[p.count] = (channel, Allowed::Listen); p.count += 1; }
    p
}
