//! AVRCP over AVCTP (PSM 0x0017) for the A2DP source (700-BLT-0007): what a sink with absolute volume asks.
//! - **Controller, category 2 (the sink's volume):** once a sink's control channel is open, GetCapabilities for its
//!   events, RegisterNotification of EVENT_VOLUME_CHANGED (registered again after each change), and SetAbsoluteVolume
//!   with the start value; the volume it reports (its own buttons) is kept and logged.
//! - **Target answers (a player, as far as a sink asks; no target record is published, as a category 1 target owes
//!   browsing and metadata this does not have):** UNIT INFO and SUBUNIT INFO (a panel), PASS THROUGH (its
//!   buttons, accepted and logged), GetCapabilities (the company ID; the events: playback status, track),
//!   RegisterNotification of those two (INTERIM now, CHANGED when the stream starts or stops), GetPlayStatus. Any
//!   other PDU is REJECTED (invalid command), any other opcode NOT IMPLEMENTED, another profile ID gets IPID.
//! Every AV/C frame is logged whole, both ways. Written from AVRCP 1.6's and AVCTP 1.4's frame formats.
use crate::buf::Hex;
use crate::ctl::Ctl;
use crate::l2cap::Channels;

/// AV Remote Control's profile ID in the AVCTP header.
pub const PID: u16 = 0x110E;
pub const UNIT_INFO: u8 = 0x30;
pub const SUBUNIT_INFO: u8 = 0x31;
pub const PASS_THROUGH: u8 = 0x7C;
pub const VENDOR_DEPENDENT: u8 = 0x00;
/// AV/C command types and response codes.
pub const CONTROL: u8 = 0x00;
pub const STATUS: u8 = 0x01;
pub const NOTIFY: u8 = 0x03;
pub const NOT_IMPLEMENTED: u8 = 0x08;
pub const ACCEPTED: u8 = 0x09;
pub const REJECTED: u8 = 0x0A;
pub const STABLE: u8 = 0x0C;
pub const CHANGED: u8 = 0x0D;
pub const INTERIM: u8 = 0x0F;
/// The panel subunit (type 9, ID 0) and the unit itself.
pub const PANEL: u8 = 9 << 3;
pub const UNIT: u8 = 0xFF;
/// The Bluetooth SIG's company ID, before every AVRCP PDU.
pub const SIG: [u8; 3] = [0x00, 0x19, 0x58];
/// AVRCP PDUs.
pub const GET_CAPABILITIES: u8 = 0x10;
pub const GET_PLAY_STATUS: u8 = 0x30;
pub const REGISTER_NOTIFICATION: u8 = 0x31;
pub const SET_ABSOLUTE_VOLUME: u8 = 0x50;
/// Capability IDs and events.
pub const COMPANY_ID: u8 = 0x02;
pub const EVENTS_SUPPORTED: u8 = 0x03;
pub const EVENT_PLAYBACK_STATUS: u8 = 0x01;
pub const EVENT_TRACK: u8 = 0x02;
pub const EVENT_VOLUME: u8 = 0x0D;
/// Error status codes.
pub const INVALID_COMMAND: u8 = 0x00;
pub const INVALID_PARAMETER: u8 = 0x01;
/// The volume given a sink at first, of 127, unless the device list names one (`volume n`).
pub const START_VOLUME: u8 = 0x40;

fn operation(id: u8) -> &'static str {
    match id & 0x7F { 0x41 => "VOLUME UP", 0x42 => "VOLUME DOWN", 0x43 => "MUTE", 0x44 => "PLAY", 0x45 => "STOP", 0x46 => "PAUSE", 0x4B => "FORWARD", 0x4C => "BACKWARD", _ => "ANOTHER" }
}

fn pdu_name(pdu: u8) -> &'static str {
    match pdu { GET_CAPABILITIES => "GET CAPABILITIES", GET_PLAY_STATUS => "GET PLAY STATUS", REGISTER_NOTIFICATION => "REGISTER NOTIFICATION", SET_ABSOLUTE_VOLUME => "SET ABSOLUTE VOLUME", _ => "ANOTHER PDU" }
}

/// What the controller side waits for and holds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Link {
    pub handle: u16, pub local: u16, pub label: u8,
    /// The sink's registrations at us (their labels): playback status, track.
    pub playback: Option<u8>, pub track: Option<u8>, pub playing: bool,
    /// Ours at the sink: its volume changes; the volume it last said; the one we set.
    pub volume_registered: bool, pub sink_volume: Option<u8>, pub set_volume: u8,
}

pub struct Control { pub link: Option<Link>, pub answered: u32, pub volume: u8 }

impl Default for Control { fn default() -> Self { Self::new() } }

/// An AVCTP frame with an AV/C body: the header (label, single packet, command or response, our profile).
fn frame(label: u8, response: bool, body: &[&[u8]], out: &mut [u8; 512]) -> usize {
    out[0] = label << 4 | (response as u8) << 1;
    out[1..3].copy_from_slice(&PID.to_be_bytes());
    let mut at = 3;
    for b in body { let n = b.len().min(512 - at); out[at..at + n].copy_from_slice(&b[..n]); at += n; }
    at
}

impl Control {
    pub const fn new() -> Self { Self { link: None, answered: 0, volume: START_VOLUME } }

    fn send(ctl: &mut Ctl, chans: &Channels, l: &Link, bytes: &[u8]) {
        ctl.say(format_args!("AVRCP: -> {}", Hex(bytes)));
        chans.send(ctl, l.handle, l.local, &[bytes]);
    }

    /// A vendor-dependent PDU: `code` (a command type, or a response code), its parameters.
    fn pdu(ctl: &mut Ctl, chans: &Channels, l: &Link, label: u8, response: bool, code: u8, pdu: u8, params: &[u8]) {
        let l16 = (params.len() as u16).to_be_bytes();
        let mut out = [0u8; 512];
        let n = frame(label, response, &[&[code, PANEL, VENDOR_DEPENDENT], &SIG, &[pdu, 0, l16[0], l16[1]], params], &mut out);
        Self::send(ctl, chans, l, &out[..n]);
    }

    /// The control channel a sink opened, or we did: our controller's first asks.
    pub fn opened(&mut self, ctl: &mut Ctl, chans: &Channels, handle: u16, local: u16, playing: bool) {
        let mut l = Link { handle, local, label: 0, playing, set_volume: self.volume, ..Link::default() };
        ctl.say(format_args!("AVRCP: CONTROL CHANNEL OPEN: ASKING THE SINK'S EVENTS"));
        l.label = 1;
        Self::pdu(ctl, chans, &l, l.label, false, STATUS, GET_CAPABILITIES, &[EVENTS_SUPPORTED]);
        self.link = Some(l);
    }

    pub fn closed(&mut self, handle: u16, local: u16) { if self.link.is_some_and(|l| l.handle == handle && l.local == local) { self.link = None; } }

    fn next_label(l: &mut Link) -> u8 { l.label = (l.label + 1) & 0x0F; l.label }

    /// The stream started or stopped: a sink registered for the playback status hears it changed.
    pub fn playing(&mut self, ctl: &mut Ctl, chans: &Channels, playing: bool) {
        let Some(mut l) = self.link else { return };
        if l.playing == playing { return; }
        l.playing = playing;
        if let Some(label) = l.playback.take() { Self::pdu(ctl, chans, &l, label, true, CHANGED, REGISTER_NOTIFICATION, &[EVENT_PLAYBACK_STATUS, playing as u8]); }
        self.link = Some(l);
    }

    /// One AVCTP frame from the sink.
    pub fn data(&mut self, ctl: &mut Ctl, chans: &Channels, handle: u16, local: u16, p: &[u8]) {
        ctl.say(format_args!("AVRCP: <- {}", Hex(p)));
        if p.len() < 3 { return; }
        let mut l = match self.link { Some(l) if l.handle == handle && l.local == local => l, _ => Link { handle, local, set_volume: self.volume, ..Link::default() } };
        let (label, kind, response) = (p[0] >> 4, p[0] >> 2 & 3, p[0] >> 1 & 1 != 0);
        if kind != 0 { self.link = Some(l); return; }
        let pid = u16::from_be_bytes([p[1], p[2]]);
        if pid != PID {
            if !response { let out = [label << 4 | 1 << 1 | 1, p[1], p[2]]; Self::send(ctl, chans, &l, &out); }
            self.link = Some(l);
            return;
        }
        if p.len() < 6 { self.link = Some(l); return; }
        let (code, subunit, opcode, operands) = (p[3] & 0x0F, p[4], p[5], &p[6..]);
        if response { self.response(ctl, chans, &mut l, code, opcode, operands); } else { self.command(ctl, chans, &mut l, label, code, subunit, opcode, operands); }
        self.link = Some(l);
    }

    /// A response to our controller's commands.
    fn response(&mut self, ctl: &mut Ctl, chans: &Channels, l: &mut Link, code: u8, opcode: u8, o: &[u8]) {
        if opcode != VENDOR_DEPENDENT || o.len() < 7 || o[..3] != SIG { return; }
        let (pdu, params) = (o[3], &o[7..]);
        match (pdu, code) {
            (GET_CAPABILITIES, STABLE) if params.len() >= 2 && params[0] == EVENTS_SUPPORTED => {
                let events = &params[2..(2 + params[1] as usize).min(params.len())];
                ctl.say(format_args!("AVRCP: THE SINK'S EVENTS {:02X?}{}", events, if events.contains(&EVENT_VOLUME) { ": IT REPORTS ITS VOLUME" } else { "" }));
                if events.contains(&EVENT_VOLUME) { self.register_volume(ctl, chans, l); }
                self.set_absolute(ctl, chans, l);
            }
            (GET_CAPABILITIES, _) => {
                ctl.say(format_args!("AVRCP: THE SINK DID NOT SAY ITS EVENTS (RESPONSE {:X}): ITS VOLUME SET ALL THE SAME", code));
                self.set_absolute(ctl, chans, l);
            }
            (REGISTER_NOTIFICATION, INTERIM | CHANGED) if params.len() >= 2 && params[0] == EVENT_VOLUME => {
                let v = params[1] & 0x7F;
                l.sink_volume = Some(v);
                ctl.say(format_args!("AVRCP: THE SINK'S VOLUME {} OF 127{}", v, if code == CHANGED { " (CHANGED THERE)" } else { "" }));
                // A notification is spent once it changed: registered again.
                if code == CHANGED { l.volume_registered = false; self.register_volume(ctl, chans, l); }
            }
            (SET_ABSOLUTE_VOLUME, ACCEPTED) if !params.is_empty() => {
                l.sink_volume = Some(params[0] & 0x7F);
                ctl.say(format_args!("AVRCP: THE SINK TOOK VOLUME {} OF 127", params[0] & 0x7F));
            }
            _ => ctl.say(format_args!("AVRCP: {} ANSWERED WITH RESPONSE {:X}", pdu_name(pdu), code)),
        }
    }

    fn register_volume(&mut self, ctl: &mut Ctl, chans: &Channels, l: &mut Link) {
        if l.volume_registered { return; }
        l.volume_registered = true;
        let label = Self::next_label(l);
        Self::pdu(ctl, chans, l, label, false, NOTIFY, REGISTER_NOTIFICATION, &[EVENT_VOLUME, 0, 0, 0, 0]);
    }

    fn set_absolute(&mut self, ctl: &mut Ctl, chans: &Channels, l: &mut Link) {
        let label = Self::next_label(l);
        ctl.say(format_args!("AVRCP: GIVING THE SINK VOLUME {} OF 127", l.set_volume));
        Self::pdu(ctl, chans, l, label, false, CONTROL, SET_ABSOLUTE_VOLUME, &[l.set_volume & 0x7F]);
    }

    /// A command from the sink to our target.
    #[allow(clippy::too_many_arguments)]
    fn command(&mut self, ctl: &mut Ctl, chans: &Channels, l: &mut Link, label: u8, ctype: u8, subunit: u8, opcode: u8, o: &[u8]) {
        let mut out = [0u8; 512];
        let n = match opcode {
            UNIT_INFO => frame(label, true, &[&[STABLE, UNIT, UNIT_INFO, 0x07, PANEL], &SIG], &mut out),
            SUBUNIT_INFO => frame(label, true, &[&[STABLE, UNIT, SUBUNIT_INFO, 0x07, PANEL, 0xFF, 0xFF, 0xFF]], &mut out),
            PASS_THROUGH if !o.is_empty() => {
                ctl.say(format_args!("AVRCP: THE SINK PRESSED {} ({:02X}, {})", operation(o[0]), o[0], if o[0] & 0x80 != 0 { "RELEASED" } else { "PRESSED" }));
                frame(label, true, &[&[ACCEPTED, subunit, PASS_THROUGH], o], &mut out)
            }
            VENDOR_DEPENDENT if o.len() >= 7 && o[..3] == SIG => {
                let (pdu, params) = (o[3], &o[7..]);
                let playing = l.playing as u8;
                match pdu {
                    GET_CAPABILITIES if params.first() == Some(&COMPANY_ID) => { Self::pdu(ctl, chans, l, label, true, STABLE, pdu, &[COMPANY_ID, 1, SIG[0], SIG[1], SIG[2]]); return; }
                    GET_CAPABILITIES if params.first() == Some(&EVENTS_SUPPORTED) => { Self::pdu(ctl, chans, l, label, true, STABLE, pdu, &[EVENTS_SUPPORTED, 2, EVENT_PLAYBACK_STATUS, EVENT_TRACK]); return; }
                    REGISTER_NOTIFICATION if params.first() == Some(&EVENT_PLAYBACK_STATUS) => {
                        l.playback = Some(label);
                        ctl.say(format_args!("AVRCP: THE SINK WATCHES THE PLAYBACK: {}", if l.playing { "PLAYING" } else { "STOPPED" }));
                        Self::pdu(ctl, chans, l, label, true, INTERIM, pdu, &[EVENT_PLAYBACK_STATUS, playing]);
                        return;
                    }
                    // No track selected (AVRCP 1.4's identifier for none).
                    REGISTER_NOTIFICATION if params.first() == Some(&EVENT_TRACK) => { l.track = Some(label); Self::pdu(ctl, chans, l, label, true, INTERIM, pdu, &[EVENT_TRACK, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); return; }
                    REGISTER_NOTIFICATION => { ctl.say(format_args!("AVRCP: THE SINK WATCHES EVENT {:02X}: NOT OURS", params.first().copied().unwrap_or(0))); Self::pdu(ctl, chans, l, label, true, REJECTED, pdu, &[INVALID_PARAMETER]); return; }
                    // Length and position unknown, the status.
                    GET_PLAY_STATUS => { Self::pdu(ctl, chans, l, label, true, STABLE, pdu, &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, playing]); return; }
                    _ => { ctl.say(format_args!("AVRCP: {} ({:02X}) NOT OURS", pdu_name(pdu), pdu)); Self::pdu(ctl, chans, l, label, true, REJECTED, pdu, &[INVALID_COMMAND]); return; }
                }
            }
            _ => {
                ctl.say(format_args!("AVRCP: OPCODE {:02X} (TYPE {}) NOT IMPLEMENTED", opcode, ctype));
                frame(label, true, &[&[NOT_IMPLEMENTED, subunit, opcode], o], &mut out)
            }
        };
        self.answered += 1;
        Self::send(ctl, chans, l, &out[..n]);
    }
}
