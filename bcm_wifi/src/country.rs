// A country's limits (wlanhw.wit 1.3, 550-NET-0035): the channels the station may use and those it may only listen on,
// as bit masks. None until first set, when 1.1's ways hold, as the station of stage A sets none (so does wlansim). EIRP
// is not applied: the transmit gain is fixed and not calibrated in dBm (550-DRV-0028).
pub const CHANNELS: core::ops::RangeInclusive<u8> = 1..=13;
const PROBE_REQUEST: u8 = 0x40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Country { channels: u16, listen: u16 }

impl Country {
    pub fn allows(c: Option<Country>, channel: u8) -> bool { c.map_or(CHANNELS.contains(&channel), |c| c.channels >> channel & 1 != 0) }

    // Whether `frame` may go out on `channel`: no probe request where the station may only listen.
    pub fn may_send(c: Option<Country>, channel: u8, frame: &[u8]) -> bool {
        let Some(c) = c else { return true };
        c.channels >> channel & 1 != 0 && !(c.listen >> channel & 1 != 0 && frame.first() == Some(&PROBE_REQUEST))
    }

    // The limits checked: two capital letters or the world's `00` (1.3), each channel once and the radio's own.
    pub fn from(code: &str, limits: impl IntoIterator<Item = (u8, bool)>) -> Option<Country> {
        if code != "00" && (code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase())) { return None; }
        let mut c = Country { channels: 0, listen: 0 };
        for (channel, listen) in limits {
            if !CHANNELS.contains(&channel) || c.channels >> channel & 1 != 0 { return None; }
            c.channels |= 1 << channel;
            if listen { c.listen |= 1 << channel; }
        }
        Some(c)
    }
}
