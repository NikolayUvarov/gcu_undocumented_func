//! A country's limits (wlanhw.wit 1.3): the channels the station may use and those it may only listen on, as bit masks.
//! None until first set, when 1.1's ways hold. `00` is the world's code, and a radio given it, or none, does not know its
//! country: its TX power is then capped at 75 % (801-DRV-0071). EIRP is not applied.

pub const CHANNELS: core::ops::RangeInclusive<u8> = 1..=13;
const PROBE_REQUEST: u8 = 0x40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Country { channels: u16, listen: u16, world: bool }

impl Country {
    pub fn allows(c: Option<Country>, ch: u8) -> bool { c.map_or(CHANNELS.contains(&ch), |c| c.channels >> ch & 1 != 0) }
    /// Whether `frame` may go out on `ch`: no probe request where the station may only listen.
    pub fn may_send(c: Option<Country>, ch: u8, frame: &[u8]) -> bool {
        let Some(c) = c else { return true };
        c.channels >> ch & 1 != 0 && !(c.listen >> ch & 1 != 0 && frame.first() == Some(&PROBE_REQUEST))
    }
    /// No country, or the world's: the TX power is capped (801-DRV-0071).
    pub fn unknown(c: Option<Country>) -> bool { c.is_none_or(|c| c.world) }
    /// The limits checked: two capital letters or the world's `00` (1.3), each channel once and the radio's own.
    pub fn from(code: &str, limits: impl IntoIterator<Item = (u8, bool)>) -> Option<Country> {
        let world = code == "00";
        if !world && (code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase())) { return None; }
        let mut c = Country { channels: 0, listen: 0, world };
        for (channel, listen) in limits {
            if !CHANNELS.contains(&channel) || c.channels >> channel & 1 != 0 { return None; }
            c.channels |= 1 << channel;
            if listen { c.listen |= 1 << channel; }
        }
        Some(c)
    }
}
