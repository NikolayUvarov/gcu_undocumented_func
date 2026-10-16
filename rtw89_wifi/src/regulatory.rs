//! Pinned frequency restrictions; selection never grants device or calibration transmission.

#[path = "regulatory_tables.rs"]
mod tables;

pub const SOURCE_COMMIT: &str = "389b9b702cf9018e9a9078ccc4fa8eaae2e054e3";
pub const SOURCE_SHA256: &str = "b71c815bd90c284131a27ab5af6fb4951b3580dc8af83345f31fcd34c5cd0581";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Provenance { pub commit: &'static str, pub db_sha256: &'static str }

pub const PROVENANCE: Provenance = Provenance { commit: SOURCE_COMMIT, db_sha256: SOURCE_SHA256 };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DfsRegion { Unspecified, Fcc, Etsi, Jp }

/// Internal named restrictions, not the Linux nl80211 or regulatory.db bit ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Flags(u16);

impl Flags {
    pub const NO_OFDM: Self = Self(1 << 0);
    pub const NO_CCK: Self = Self(1 << 1);
    pub const NO_INDOOR: Self = Self(1 << 2);
    pub const NO_OUTDOOR: Self = Self(1 << 3);
    pub const DFS: Self = Self(1 << 4);
    pub const PTP_ONLY: Self = Self(1 << 5);
    pub const PTMP_ONLY: Self = Self(1 << 6);
    pub const NO_IR: Self = Self(1 << 7);
    pub const NO_HT40: Self = Self(1 << 8);
    pub const AUTO_BW: Self = Self(1 << 9);

    pub const fn contains(self, restriction: Self) -> bool { self.0 & restriction.0 == restriction.0 }
    pub const fn is_empty(self) -> bool { self.0 == 0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmmAc { pub cw_min: u16, pub cw_max: u16, pub aifsn: u8, pub cot_ms: u16 }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wmm {
    pub name: &'static str,
    /// VO/VI/BE/BK client, followed by VO/VI/BE/BK AP, in upstream order.
    pub categories: [WmmAc; 8],
}

/// A source rule in any band, retained for provenance, not all-band driver support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rule {
    pub start_khz: u32,
    pub end_khz: u32,
    pub max_bandwidth_khz: u32,
    pub max_eirp_mbm: i32,
    pub flags: Flags,
    pub wmm: Option<&'static Wmm>,
}

impl Rule {
    /// Floor EIRP to half-dBm; this is neither conducted power nor an antenna/SAR value.
    pub const fn eirp_half_dbm(&self) -> i32 { self.max_eirp_mbm.div_euclid(50) }

    pub const fn eirp_quarter_dbm(&self) -> i32 { self.max_eirp_mbm.div_euclid(25) }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleRef { index: u16, source_line: u16 }

#[derive(Debug)]
pub struct Country {
    alpha2: [u8; 2],
    dfs: DfsRegion,
    source_line: u16,
    rules: &'static [RuleRef],
}

impl Country {
    pub const fn alpha2(&self) -> [u8; 2] { self.alpha2 }
    pub const fn dfs_region(&self) -> DfsRegion { self.dfs }
    pub const fn source_line(&self) -> u16 { self.source_line }
    pub const fn is_world(&self) -> bool { self.alpha2[0] == b'0' && self.alpha2[1] == b'0' }

    /// All source bands, including unsupported modes and NO-INDOOR records.
    pub fn source_rules(&self) -> impl ExactSizeIterator<Item = (&'static Rule, u16)> + '_ {
        self.rules.iter().map(|reference| (&tables::RULES[reference.index as usize], reference.source_line))
    }

    /// The complete center +/-10 MHz interval must fit one source rule.
    pub fn channel_2g20(&self, number: u8) -> Result<Channel, Error> {
        let center_khz = match number {
            1..=13 => 2_407_000 + u32::from(number) * 5_000,
            14 => 2_484_000,
            _ => return Err(Error::Channel),
        };
        let mut selected = None;
        for (rule, source_line) in self.source_rules() {
            if rule.max_bandwidth_khz >= 20_000 && rule.start_khz <= center_khz - 10_000
                && center_khz + 10_000 <= rule.end_khz {
                if selected.is_some() { return Err(Error::AmbiguousRule); }
                selected = Some(Channel { country: self.alpha2, number, center_khz, rule, source_line });
            }
        }
        selected.ok_or(Error::OutsideRules)
    }

    /// Covered channels, including passive/conditional ones; inspect their restrictions.
    pub fn channels_2g20(&self) -> impl Iterator<Item = Channel> + '_ {
        (1..=14).filter_map(|number| self.channel_2g20(number).ok())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { CountryFormat, UnknownCountry, WorldIsNotCountry, Channel, OutsideRules, AmbiguousRule }

/// An explicit operator choice, with no default, persistence or transmission authority.
#[derive(Clone, Copy, Debug)]
pub struct Selection(&'static Country);

impl Selection {
    pub const fn country(self) -> &'static Country { self.0 }
    pub const fn provenance(self) -> Provenance { PROVENANCE }
}

/// Every source entry, sorted by code; includes historical AN and special world 00.
pub fn countries() -> &'static [Country] { tables::COUNTRIES }

pub fn select(alpha2: [u8; 2]) -> Result<Selection, Error> {
    if alpha2 == *b"00" { return Err(Error::WorldIsNotCountry); }
    if !alpha2.iter().all(u8::is_ascii_uppercase) { return Err(Error::CountryFormat); }
    let index = tables::COUNTRIES.binary_search_by_key(&alpha2, |country| country.alpha2)
        .map_err(|_| Error::UnknownCountry)?;
    Ok(Selection(&tables::COUNTRIES[index]))
}

/// Read world restrictions without treating them as an operator-selected country.
pub fn world() -> &'static Country { &tables::COUNTRIES[0] }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Environment { Unknown, Indoor, Outdoor }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Restriction { DriverChannel, Dfs, Modulation, Topology, Wmm, Environment }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Assessment {
    /// Still needs actual board, chip tables, firmware provenance, gain and platform SAR intersection.
    EligibleForIntersection,
    PassiveOnly,
    Unsupported(Restriction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Channel {
    pub country: [u8; 2],
    pub number: u8,
    pub center_khz: u32,
    pub rule: &'static Rule,
    pub source_line: u16,
}

impl Channel {
    /// Initial RTL8852BE 2G20 feature intersection; this result cannot mint a Rule or TX token.
    pub fn assess_initial_driver(&self, environment: Environment) -> Assessment {
        let flags = self.rule.flags;
        if self.number > 13 { return Assessment::Unsupported(Restriction::DriverChannel); }
        if flags.contains(Flags::DFS) { return Assessment::Unsupported(Restriction::Dfs); }
        if flags.contains(Flags::NO_OFDM) || flags.contains(Flags::NO_CCK) {
            return Assessment::Unsupported(Restriction::Modulation);
        }
        if flags.contains(Flags::PTP_ONLY) || flags.contains(Flags::PTMP_ONLY) {
            return Assessment::Unsupported(Restriction::Topology);
        }
        if self.rule.wmm.is_some() { return Assessment::Unsupported(Restriction::Wmm); }
        if (flags.contains(Flags::NO_OUTDOOR) && environment != Environment::Indoor)
            || (flags.contains(Flags::NO_INDOOR) && environment != Environment::Outdoor) {
            return Assessment::Unsupported(Restriction::Environment);
        }
        if flags.contains(Flags::NO_IR) { return Assessment::PassiveOnly; }
        Assessment::EligibleForIntersection
    }
}
