//! The station's country limits (wlanhw.wit 1.3) under the radio's own catalogue; no transmission authority by itself.

use crate::{band, regulatory::{self, Flags, OwnerRule, Source}};

/// The radio's own channels, 20 MHz each: 2.4 GHz 1 to 13 and the 5 GHz ones (213-DRV-0087).
pub const CHANNELS: [u8; 41] = band::SUPPORTED;

/// `Invalid`: a code that is not two capital letters nor `00`, a channel twice or not the radio's.
/// `Failed`: a well-formed code the radio's catalogue does not hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal { Invalid, Failed }

/// One channel as the radio applies it: EIRP in half-dBm, and whether it may only listen there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Applied { pub channel: u8, pub eirp_half_dbm: i8, pub listen: bool }

/// One accepted `set-country`; `version` numbers the settings so that an older one cannot be mistaken for it.
#[derive(Clone, Copy, Debug)]
pub struct Limits { code: [u8; 2], version: u64, applied: [Option<Applied>; 178], by_owner: [bool; 178] }

impl Limits {
    /// The station's EIRP is in whole dBm; the catalogue's rule for the same code caps it, and its NO-IR (and on 5 GHz its
    /// DFS) keeps a channel listen-only. A channel the catalogue or this driver's 20 MHz scope does not allow is left out.
    pub fn accept(code: &str, version: u64, limits: impl IntoIterator<Item = (u8, u8, bool)>) -> Result<Self, Refusal> {
        Self::accept_with(code, version, limits, &[])
    }
    /// The same with the owner's rules beside the catalogue (550-DRV-0088): a channel no catalogue rule of the country
    /// touches may be planned from one of them (regdb's Country::plan), under the radio's own exclusions as a catalogue
    /// rule is. Nothing hands the radio owner's rules yet: that waits for the station's side (NET).
    pub fn accept_with(code: &str, version: u64, limits: impl IntoIterator<Item = (u8, u8, bool)>, owner: &[OwnerRule])
        -> Result<Self, Refusal> {
        let bytes: [u8; 2] = code.as_bytes().try_into().map_err(|_| Refusal::Invalid)?;
        let world = bytes == *b"00";
        if !world && !bytes.iter().all(u8::is_ascii_uppercase) { return Err(Refusal::Invalid); }
        if version == 0 { return Err(Refusal::Failed); }
        let country = if world { regulatory::world() } else { regulatory::select(bytes).map_err(|_| Refusal::Failed)?.country() };
        let mut applied = [None; 178]; let mut by_owner = [false; 178];
        for (channel, eirp, listen) in limits {
            if !CHANNELS.contains(&channel) || applied[usize::from(channel)].is_some() { return Err(Refusal::Invalid); }
            let Some((rule, passive, owners)) = rule(country, owner, channel) else {
                // A channel left out is still marked seen, so that a second mention of it is refused.
                applied[usize::from(channel)] = Some(None); continue
            };
            let cap = rule.eirp_half_dbm().clamp(-64, 63);
            let eirp_half_dbm = (i32::from(eirp) * 2).min(cap) as i8;
            applied[usize::from(channel)] = Some(Some(Applied { channel, eirp_half_dbm, listen: listen || passive }));
            by_owner[usize::from(channel)] = owners;
        }
        Ok(Self { code: bytes, version, applied: applied.map(Option::flatten), by_owner })
    }
    /// The world rules (00) at their own EIRP, for a radio the station gave no country (801-DRV-0071).
    pub fn world(version: u64) -> Result<Self, Refusal> { Self::accept("00", version, CHANNELS.map(|ch| (ch, u8::MAX, false))) }
    pub fn code(&self) -> [u8; 2] { self.code }
    pub fn version(&self) -> u64 { self.version }
    /// The channels left after the radio's own limits, for `info`.
    pub fn channels(&self) -> impl Iterator<Item = u8> + '_ { self.applied.iter().flatten().map(|a| a.channel) }
    pub fn applied(&self, channel: u8) -> Option<Applied> { self.applied.get(usize::from(channel)).copied().flatten() }
    /// Whether an applied channel's rule is the owner's, not the catalogue's (550-DRV-0088), to be named so where it acts.
    pub fn by_owner(&self, channel: u8) -> bool { self.applied(channel).is_some() && self.by_owner[usize::from(channel)] }
}

/// The rule for one of the radio's channels, whether it keeps the channel listen-only, and whether it is the owner's;
/// None leaves it out. regdb's Country::plan gives the one rule that holds the whole 20 MHz channel: the catalogue's,
/// or an owner's where no catalogue rule of the country touches the channel (550-DRV-0088). 2.4 GHz keeps regdb's
/// initial assessment (Channel::assess_initial_driver, here for a rule that may be the owner's). On 5 GHz DFS and NO-IR
/// make it listen-only (the station sends there only to an access point it heard); a WMM rule (the driver does not set
/// the channel-access limits it asks for), an indoor or outdoor restriction (the environment is unknown), one
/// modulation or a point-to-point rule leave it out.
fn rule(country: &regulatory::Country, owner: &[OwnerRule], channel: u8) -> Option<(regulatory::Rule, bool, bool)> {
    let centre = u32::from(band::center_mhz(channel)?) * 1000;
    let planned = country.plan(owner, centre, 20_000).ok()?;
    let (rule, owners) = (planned.rule, matches!(planned.source, Source::Owner { .. }));
    let f = rule.flags;
    match band::band(channel)? {
        band::Band::Ghz2 => {
            let refused = [Flags::DFS, Flags::NO_OFDM, Flags::NO_CCK, Flags::PTP_ONLY, Flags::PTMP_ONLY, Flags::NO_OUTDOOR, Flags::NO_INDOOR];
            if channel > 13 || rule.wmm.is_some() || refused.iter().any(|&r| f.contains(r)) { return None; }
            Some((rule, f.contains(Flags::NO_IR), owners))
        }
        band::Band::Ghz5 => {
            let refused = [Flags::NO_OFDM, Flags::PTP_ONLY, Flags::PTMP_ONLY, Flags::NO_INDOOR, Flags::NO_OUTDOOR];
            if rule.wmm.is_some() || refused.iter().any(|&r| f.contains(r)) { return None; }
            Some((rule, f.contains(Flags::DFS) || f.contains(Flags::NO_IR), owners))
        }
    }
}
