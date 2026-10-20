//! A country's rules on the radio's channels (550-NET-0035): where the station may send, where it may only listen, and
//! where it may not go. The rules come from the regulatory catalogue (wireless-regdb) through the station service; this
//! module only reads them, so a channel is never allowed by default.

/// A catalogue rule as the station reads it: the frequency range, the widest channel, the most EIRP (mBm, 100 per dBm)
/// and the restrictions it keeps or refuses, by wireless-regdb's names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rule {
    pub start_khz: u32,
    pub end_khz: u32,
    pub bandwidth_khz: u32,
    pub eirp_mbm: i32,
    /// NO-IR: no initiating radiation, so no probe request.
    pub no_ir: bool,
    /// DFS: radar detection is the access point's; a station only listens before it joins.
    pub dfs: bool,
    /// NO-OFDM or NO-CCK: one modulation alone, which the station's rates do not keep to.
    pub one_modulation: bool,
    /// NO-INDOOR or NO-OUTDOOR: the station does not know which it is.
    pub one_environment: bool,
    /// PTP-ONLY or PTMP-ONLY: not a station's use.
    pub point_to_point: bool,
}

/// A channel's standing under a country (wifi.wit 1.1 `standing`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// The station may probe and join there.
    Send,
    /// The station only listens there, and joins an access point it heard (NO-IR, DFS).
    Listen,
    /// Outside the rules, covered by two of them, or restricted in a way the station does not keep.
    Barred,
    /// The rules allow it; the radio does not.
    Radio,
}

/// The centre of a 20 MHz channel in kHz: 1 to 14 on 2.4 GHz, 32 to 177 on 5 GHz; None for any other number (184 to 196
/// are Japan's 4.9 GHz channels, which the station does not use).
pub fn centre_khz(channel: u8) -> Option<u32> {
    match channel {
        1..=13 => Some(2_407_000 + 5_000 * channel as u32),
        14 => Some(2_484_000),
        32..=177 => Some(5_000_000 + 5_000 * channel as u32),
        _ => None,
    }
}

/// The one rule that holds the whole 20 MHz channel, if exactly one does.
pub fn rule_for(rules: &[Rule], channel: u8) -> Option<Rule> {
    let centre = centre_khz(channel)?;
    let mut holding = rules.iter().filter(|r| r.bandwidth_khz >= 20_000 && r.start_khz + 10_000 <= centre && centre + 10_000 <= r.end_khz);
    let rule = *holding.next()?;
    holding.next().is_none().then_some(rule)
}

/// The standing of `channel` under `rules`, where `radio` says whether the radio can use it, and the most EIRP the rules
/// allow there in dBm (rounded down; 0 when barred).
pub fn standing(rules: &[Rule], channel: u8, radio: bool) -> (Standing, u8) {
    let Some(rule) = rule_for(rules, channel) else { return (Standing::Barred, 0) };
    if rule.one_modulation || rule.one_environment || rule.point_to_point { return (Standing::Barred, 0); }
    let eirp = rule.eirp_mbm.div_euclid(100).clamp(0, u8::MAX as i32) as u8;
    if !radio { return (Standing::Radio, eirp); }
    (if rule.no_ir || rule.dfs { Standing::Listen } else { Standing::Send }, eirp)
}
