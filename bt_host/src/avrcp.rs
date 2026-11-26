//! AVRCP's smallest honest part (AVRCP 1.0 over AVCTP 1.4, PSM 0x0017): a sink that opens the control channel to its
//! source is answered as by a target with one panel and no metadata. UNIT INFO and SUBUNIT INFO say "panel"; PASS
//! THROUGH (its buttons: play, pause, volume) is accepted and logged; everything else, the vendor-dependent metadata of
//! AVRCP 1.3 and later (absolute volume among it) included, is NOT IMPLEMENTED; another profile ID gets IPID.
use crate::ctl::Ctl;
use crate::l2cap::Channels;

/// AV Remote Control's profile ID in the AVCTP header.
pub const PID: u16 = 0x110E;
pub const UNIT_INFO: u8 = 0x30;
pub const SUBUNIT_INFO: u8 = 0x31;
pub const PASS_THROUGH: u8 = 0x7C;
pub const VENDOR_DEPENDENT: u8 = 0x00;
/// AV/C response codes.
pub const NOT_IMPLEMENTED: u8 = 0x08;
pub const ACCEPTED: u8 = 0x09;
pub const STABLE: u8 = 0x0C;
/// The panel subunit (type 9, ID 0) and the unit itself.
pub const PANEL: u8 = 9 << 3;
pub const UNIT: u8 = 0xFF;

/// A pass-through operation's name, for the log.
fn operation(id: u8) -> &'static str {
    match id & 0x7F { 0x41 => "VOLUME UP", 0x42 => "VOLUME DOWN", 0x43 => "MUTE", 0x44 => "PLAY", 0x45 => "STOP", 0x46 => "PAUSE", 0x4B => "FORWARD", 0x4C => "BACKWARD", _ => "ANOTHER" }
}

/// The answer to one AVCTP command into `out`; its length (0: nothing to answer, a response or a fragment).
pub fn answer(p: &[u8], out: &mut [u8; 512], mut said: impl FnMut(core::fmt::Arguments)) -> usize {
    if p.len() < 3 { return 0; }
    let (label, kind, response) = (p[0] >> 4, p[0] >> 2 & 3, p[0] >> 1 & 1);
    // Single packets only: AV/C commands are short.
    if kind != 0 || response != 0 { return 0; }
    let pid = u16::from_be_bytes([p[1], p[2]]);
    out[0] = label << 4 | 1 << 1;
    out[1..3].copy_from_slice(&p[1..3]);
    if pid != PID { out[0] |= 1; return 3; }
    if p.len() < 6 { return 0; }
    let (ctype, subunit, opcode, operands) = (p[3] & 0x0F, p[4], p[5], &p[6..]);
    let (code, body): (u8, &[u8]) = match opcode {
        UNIT_INFO => (STABLE, &[0x07, PANEL, 0x00, 0x19, 0x58]),
        SUBUNIT_INFO => (STABLE, &[0x07, PANEL, 0xFF, 0xFF, 0xFF]),
        PASS_THROUGH if !operands.is_empty() => {
            said(format_args!("AVRCP: THE SINK PRESSED {} ({:02X}, {})", operation(operands[0]), operands[0], if operands[0] & 0x80 != 0 { "RELEASED" } else { "PRESSED" }));
            (ACCEPTED, operands)
        }
        _ => {
            said(format_args!("AVRCP: COMMAND {:02X} (TYPE {}) NOT IMPLEMENTED", opcode, ctype));
            (NOT_IMPLEMENTED, operands)
        }
    };
    let n = body.len().min(500);
    out[3..6].copy_from_slice(&[code, if opcode == UNIT_INFO || opcode == SUBUNIT_INFO { UNIT } else { subunit }, opcode]);
    out[6..6 + n].copy_from_slice(&body[..n]);
    6 + n
}

/// The control channel of each link that opened one.
pub struct Control { pub answered: u32 }

impl Default for Control { fn default() -> Self { Self::new() } }

impl Control {
    pub const fn new() -> Self { Self { answered: 0 } }

    pub fn pdu(&mut self, ctl: &mut Ctl, chans: &Channels, handle: u16, local: u16, p: &[u8]) {
        let mut out = [0u8; 512];
        let n = answer(p, &mut out, |line| ctl.say(line));
        if n > 0 { self.answered += 1; chans.send(ctl, handle, local, &[&out[..n]]); }
    }
}
