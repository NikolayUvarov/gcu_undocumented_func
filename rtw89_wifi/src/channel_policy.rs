// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Pure pinned default-table decoding; none of these values authorizes a channel or transmission.

use crate::board::Board;
use crate::power_tables as tables;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { Board, Index, PowerRange }

/// A chip power-table group, not a frequency regulatory domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Group(u8);

impl Group {
    pub const WORLDWIDE: Self = Self(0);
    pub const ETSI: Self = Self(1);
    pub const FCC: Self = Self(2);
    pub const MKK: Self = Self(3);
    pub const UK: Self = Self(14);
    pub const fn from_id(id: u8) -> Option<Self> { if id < 16 { Some(Self(id)) } else { None } }
    pub const fn id(self) -> u8 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountryDefault {
    pub alpha2: [u8; 2],
    pub groups: [Group; 3],
    pub function_flags: u8,
}

/// Source defaults only; firmware elements and platform UK/ETSI policy can override these groups.
pub fn country_default(alpha2: [u8; 2]) -> Option<CountryDefault> {
    let at = tables::COUNTRIES.binary_search_by_key(&alpha2, |row| row.0).ok()?;
    let (alpha2, groups, function_flags) = tables::COUNTRIES[at];
    Some(CountryDefault { alpha2, groups: [Group(groups[0]), Group(groups[1]), Group(groups[2])], function_flags })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hint {
    /// Upstream requests strict country policy and ignores country IEs; external frequency rules are still needed.
    Programmed(CountryDefault),
    /// Worldwide, erased and unknown bytes follow the external stack policy; no country has been selected here.
    ExternalPolicyRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardCountry { pub raw: [u8; 2], pub raw_channel_plan: u8, pub hint: Hint }

pub fn board_country(board: &Board) -> Result<BoardCountry, Error> {
    if board.identity.cut != 1 || board.identity.rfe != 1 { return Err(Error::Board); }
    let raw = board.identity.country;
    Ok(BoardCountry { raw, raw_channel_plan: board.identity.channel_plan,
        hint: country_default(raw).map(Hint::Programmed).unwrap_or(Hint::ExternalPolicyRequired) })
}

/// Preserve explicit C zeros separately from unspecified entries; both have raw value zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry { pub value: i8, pub explicit: bool }

fn entry(rows: &[(u16, i8)], index: u16) -> Entry {
    match rows.binary_search_by_key(&index, |row| row.0) {
        Ok(at) => Entry { value: rows[at].1, explicit: true },
        Err(_) => Entry { value: 0, explicit: false },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultLimit {
    pub requested: Entry,
    pub worldwide: Entry,
    pub used_worldwide: bool,
    pub quarter_dbm: i8,
}

fn fallback(requested: Entry, worldwide: Entry) -> DefaultLimit {
    let used_worldwide = requested.value == 0;
    DefaultLimit { requested, worldwide, used_worldwide,
        quarter_dbm: if used_worldwide { worldwide.value } else { requested.value } }
}

fn channel_index(channel: u8) -> Result<u16, Error> {
    if !(1..=14).contains(&channel) { return Err(Error::Index); }
    Ok(channel as u16 - 1)
}

/// Decode all source 2 GHz slots, including CCK-40 fields needed even by a 20 MHz power image.
pub fn default_limit_2g(bandwidth_index: u8, streams: u8, rate_section: u8, beamformed: bool,
                        group: Group, channel: u8) -> Result<DefaultLimit, Error> {
    if bandwidth_index > 1 || !(1..=2).contains(&streams) || rate_section > 2 { return Err(Error::Index); }
    let ch = channel_index(channel)?;
    let base = (((bandwidth_index as u16 * 2 + streams as u16 - 1) * 3 + rate_section as u16) * 2
        + beamformed as u16) * 16 * 14;
    Ok(fallback(entry(tables::LIMIT_2G, base + group.0 as u16 * 14 + ch), entry(tables::LIMIT_2G, base + ch)))
}

/// RTL8852B uses RU26, RU52 and RU106; newer combined RU formats are refused.
pub fn default_ru_2g(ru_index: u8, streams: u8, group: Group, channel: u8) -> Result<DefaultLimit, Error> {
    if ru_index > 2 || !(1..=2).contains(&streams) { return Err(Error::Index); }
    let ch = channel_index(channel)?;
    let base = (ru_index as u16 * 2 + streams as u16 - 1) * 16 * 14;
    Ok(fallback(entry(tables::RU_2G, base + group.0 as u16 * 14 + ch), entry(tables::RU_2G, base + ch)))
}

/// Return quarter-dBm by-rate data, including the source's distinct five rate-offset slots.
pub fn default_byrate_2g(streams: u8, rate_section: u8, index: u8) -> Result<Entry, Error> {
    let counts = [4, 8, 12, 4, 5];
    if !(1..=2).contains(&streams) || rate_section > 4 || index >= counts[rate_section as usize]
        || (matches!(rate_section, 0 | 1 | 4) && streams != 1) { return Err(Error::Index); }
    Ok(entry(tables::BYRATE_2G, ((streams as u16 - 1) * 5 + rate_section as u16) * 12 + index as u16))
}

pub fn default_shape_2g(rate_section: u8, group: Group) -> Result<Entry, Error> {
    if rate_section > 1 { return Err(Error::Index); }
    Ok(entry(tables::SHAPE_2G, rate_section as u16 * 16 + group.0 as u16))
}

pub fn default_ru_shape_2g(group: Group) -> Entry { entry(tables::SHAPE_RU_2G, group.0 as u16) }

/// Arithmetic right shift preserves the source's downward rounding for negative quarter-dBm values.
pub const fn rf_to_mac(quarter_dbm: i8) -> i8 { quarter_dbm >> 1 }

pub fn dbm_to_mac(dbm: i8) -> i8 { (dbm as i16 * 2).clamp(-64, 63) as i8 }

/// Pure arithmetic for explicit inputs; callers must establish policy provenance outside this decoder.
pub fn constrained_mac(quarter_dbm: i8, antenna_offset: i8, antenna_limit: Option<i8>,
                       sar_half_dbm: i8, tpe_half_dbm: i8) -> Result<i8, Error> {
    if !(-64..=63).contains(&sar_half_dbm) || !(-64..=63).contains(&tpe_half_dbm) { return Err(Error::PowerRange); }
    let adjusted = match antenna_limit.filter(|v| *v != 0 && *v != 127) {
        Some(limit) => (quarter_dbm as i16 + antenna_offset as i16).min(limit as i16),
        None => quarter_dbm as i16,
    };
    let adjusted = i8::try_from(adjusted).map_err(|_| Error::PowerRange)?;
    Ok(rf_to_mac(adjusted).min(sar_half_dbm).min(tpe_half_dbm))
}
