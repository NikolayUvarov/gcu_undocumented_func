//! A hub's facts as usb_host uses them (USB 2.0 chapter 11, USB 3.2 chapter 10): the descriptors, the port status
//! bits, the speed after a reset and the change bits to clear. Plain logic: tests/usb_hub_host.rs.

/// Class requests to a hub and the feature selectors of its ports.
pub const SET_HUB_DEPTH: u8 = 12;
pub const HUB_DESCRIPTOR: u16 = 0x2900;
pub const SUPERSPEED_HUB_DESCRIPTOR: u16 = 0x2A00;
pub const PORT_RESET: u16 = 4;
pub const PORT_POWER: u16 = 8;
pub const C_PORT_CONNECTION: u16 = 16;
pub const C_PORT_RESET: u16 = 20;
pub const BH_PORT_RESET: u16 = 28;
pub const C_BH_PORT_RESET: u16 = 29;

/// The port status bits both kinds share: a device there, the port enabled.
pub const CONNECTED: u16 = 1;
pub const ENABLED: u16 = 2;
/// The port change bits: connection, the reset done, and (a SuperSpeed hub's) the warm reset done.
pub const CHANGED_CONNECTION: u16 = 1;
pub const CHANGED_RESET: u16 = 0x10;
pub const CHANGED_WARM_RESET: u16 = 0x20;

/// A hub as its descriptor gives it: its ports (at most 15, a route string's digit), its think time (a high-speed
/// hub's TT; 0 otherwise) and the time its ports take to power up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hub { pub ports: u8, pub think: u8, pub power_ms: u64 }

/// A USB 2 hub's descriptor (type 0x29): ports at 2, the TT think time in bits 5-6 of 3, power on to good at 5.
pub fn usb2(d: &[u8]) -> Option<Hub> {
    if d.len() < 7 || d[1] != 0x29 { return None; }
    Some(Hub { ports: d[2].min(15), think: (d[3] >> 5) & 3, power_ms: d[5] as u64 * 2 })
}

/// A SuperSpeed hub's descriptor (type 0x2A, 12 bytes): ports at 2, power on to good at 5; no TT.
pub fn superspeed(d: &[u8]) -> Option<Hub> {
    if d.len() < 12 || d[1] != 0x2A { return None; }
    Some(Hub { ports: d[2].min(15), think: 0, power_ms: d[5] as u64 * 2 })
}

/// The speed of the device on a port just reset, as the xHCI slot context codes it (1 full, 2 low, 3 high,
/// 4 SuperSpeed). A SuperSpeed hub's ports carry only SuperSpeed devices; its bit 9 is the port's power, not low speed.
pub fn speed_after_reset(superspeed_hub: bool, status: u16) -> u8 {
    if superspeed_hub { 4 } else if status & 0x200 != 0 { 2 } else if status & 0x400 != 0 { 3 } else { 1 }
}

/// The feature selectors that clear a port's other change bits, so the hub stops reporting the port: on a USB 2 hub
/// enable (1), suspend (2) and over-current (3); on a SuperSpeed hub over-current (3), warm reset (5), link state (6)
/// and configuration error (7).
pub fn other_changes(superspeed_hub: bool, change: u16) -> impl Iterator<Item = u16> {
    const USB2: [(u16, u16); 3] = [(1 << 1, 17), (1 << 2, 18), (1 << 3, 19)];
    const SUPERSPEED: [(u16, u16); 4] = [(1 << 3, 19), (1 << 5, 29), (1 << 6, 25), (1 << 7, 26)];
    let table: &'static [(u16, u16)] = if superspeed_hub { &SUPERSPEED } else { &USB2 };
    table.iter().filter(move |&&(bit, _)| change & bit != 0).map(|&(_, feature)| feature)
}
