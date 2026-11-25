//! The LE central: links to the LE devices listed (HID over GATT), their security and their attribute protocol.
//! 700-BLT-0005 makes only the link: pairing (SMP), the ATT client, GATT discovery and HID over GATT are 700-BLT-0009's,
//! not written yet. Meanwhile a peer's ATT requests are answered "not supported" so that it does not wait on us.
use crate::buf::Addr;
use crate::ctl::Ctl;
use crate::devices::{Devices, Ltk};
use crate::l2cap;
use crate::stack::Ups;

/// ATT's Error Response and its "request not supported" code.
pub const ATT_ERROR: u8 = 0x01;
pub const REQUEST_NOT_SUPPORTED: u8 = 0x06;

pub struct Central { pub links: u32 }

impl Default for Central { fn default() -> Self { Self::new() } }

/// LE Create Connection's parameters for `addr`: scan every 60 ms for 30 ms, no filter list, a 30-50 ms connection
/// interval, no latency, a 5 s supervision timeout.
pub fn create_connection(addr: Addr, random: bool) -> [u8; 25] {
    let mut b = [0u8; 25];
    b[..4].copy_from_slice(&[0x60, 0x00, 0x30, 0x00]);
    b[4] = 0;
    b[5] = random as u8;
    b[6..12].copy_from_slice(&addr.0);
    b[12] = 0;
    for (i, v) in [0x0018u16, 0x0028, 0, 0x01F4, 0, 0].iter().enumerate() { b[13 + 2 * i..15 + 2 * i].copy_from_slice(&v.to_le_bytes()); }
    b
}

impl Central {
    pub const fn new() -> Self { Self { links: 0 } }

    pub fn link_up(&mut self, ctl: &mut Ctl, handle: u16, addr: Addr, _ltk: Option<Ltk>) {
        self.links += 1;
        ctl.say(format_args!("LE {:03X} ({}): THE LINK IS UP; PAIRING AND HID OVER GATT ARE NOT WRITTEN YET (700-BLT-0009)", handle, addr));
    }

    pub fn link_down(&mut self, _handle: u16) {}

    pub fn encrypted(&mut self, ctl: &mut Ctl, handle: u16, status: u8, on: bool) {
        ctl.say(format_args!("LE {:03X}: ENCRYPTION {} (STATUS {:02X})", handle, if on { "ON" } else { "OFF" }, status));
    }

    /// An ATT or SMP PDU.
    pub fn pdu(&mut self, ctl: &mut Ctl, _ups: &mut Ups, _devices: &mut Devices, handle: u16, cid: u16, p: &[u8]) {
        let Some(&op) = p.first() else { return };
        if cid == l2cap::ATT {
            // A request (not a command, notification, indication or response): not supported.
            let request = op & 0x40 == 0 && op & 1 == 0 && !matches!(op, 0x1B | 0x1D);
            if request { ctl.l2cap(handle, l2cap::ATT, &[&[ATT_ERROR, op, p.get(1).copied().unwrap_or(0), p.get(2).copied().unwrap_or(0), REQUEST_NOT_SUPPORTED]]); }
        } else if cid == l2cap::SMP {
            // Pairing not supported (0x05).
            ctl.l2cap(handle, l2cap::SMP, &[&[0x05, 0x05]]);
        }
    }

    pub fn tick(&mut self, _ctl: &mut Ctl) {}
}
