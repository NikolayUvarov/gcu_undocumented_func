//! The station's country limits (wlanhw.wit 1.3) under the radio's own catalogue; no transmission authority by itself.

use crate::regulatory::{self, Assessment, Environment};

/// The radio's own channels: 2.4 GHz, 20 MHz.
pub const CHANNELS: core::ops::RangeInclusive<u8> = 1..=13;

/// `Invalid`: a code that is not two capital letters nor `00`, a channel twice or not the radio's.
/// `Failed`: a well-formed code the radio's catalogue does not hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal { Invalid, Failed }

/// One channel as the radio applies it: EIRP in half-dBm, and whether it may only listen there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Applied { pub channel: u8, pub eirp_half_dbm: i8, pub listen: bool }

/// One accepted `set-country`; `version` numbers the settings so that an older one cannot be mistaken for it.
#[derive(Clone, Copy, Debug)]
pub struct Limits { code: [u8; 2], version: u64, applied: [Option<Applied>; 14] }

impl Limits {
    /// The station's EIRP is in whole dBm; the catalogue's rule for the same code caps it, and its NO-IR keeps a channel
    /// listen-only. A channel the catalogue or this driver's 2G20 scope does not allow is left out.
    pub fn accept(code: &str, version: u64, limits: impl IntoIterator<Item = (u8, u8, bool)>) -> Result<Self, Refusal> {
        let bytes: [u8; 2] = code.as_bytes().try_into().map_err(|_| Refusal::Invalid)?;
        let world = bytes == *b"00";
        if !world && !bytes.iter().all(u8::is_ascii_uppercase) { return Err(Refusal::Invalid); }
        if version == 0 { return Err(Refusal::Failed); }
        let country = if world { regulatory::world() } else { regulatory::select(bytes).map_err(|_| Refusal::Failed)?.country() };
        let mut seen = 0u16;
        let mut applied = [None; 14];
        for (channel, eirp, listen) in limits {
            if !CHANNELS.contains(&channel) || seen >> channel & 1 != 0 { return Err(Refusal::Invalid); }
            seen |= 1 << channel;
            let Ok(rule) = country.channel_2g20(channel) else { continue };
            let passive = match rule.assess_initial_driver(Environment::Unknown) {
                Assessment::EligibleForIntersection => false,
                Assessment::PassiveOnly => true,
                Assessment::Unsupported(_) => continue,
            };
            let cap = rule.rule.eirp_half_dbm().clamp(-64, 63);
            let eirp_half_dbm = (i32::from(eirp) * 2).min(cap) as i8;
            applied[usize::from(channel)] = Some(Applied { channel, eirp_half_dbm, listen: listen || passive });
        }
        Ok(Self { code: bytes, version, applied })
    }
    /// The world rules (00) at their own EIRP, for a radio the station gave no country (801-DRV-0071).
    pub fn world(version: u64) -> Result<Self, Refusal> { Self::accept("00", version, CHANNELS.map(|ch| (ch, u8::MAX, false))) }
    pub fn code(&self) -> [u8; 2] { self.code }
    pub fn version(&self) -> u64 { self.version }
    /// The channels left after the radio's own limits, for `info`.
    pub fn channels(&self) -> impl Iterator<Item = u8> + '_ { self.applied.iter().flatten().map(|a| a.channel) }
    pub fn applied(&self, channel: u8) -> Option<Applied> { self.applied.get(usize::from(channel)).copied().flatten() }
}
