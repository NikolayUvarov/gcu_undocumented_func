//! The host (700-BLT-0005, 0006): the controller set up for links, connections made and taken (BR/EDR and LE), their
//! security (Secure Simple Pairing with the controller doing the ECDH, legacy PIN "0000", link keys kept), their L2CAP
//! PDUs reassembled and handed to the channels and profiles. Bytes in (events, ACL packets, the time), bytes out
//! (commands and ACL packets for the controller, log lines, upcalls for the driver): no I/O of its own.
use crate::acl::{self, Pdu};
use crate::avdtp;
use crate::buf::{le16, Addr, Queue};
use crate::ctl::Ctl;
use crate::devices::{Devices, Kind, Ltk};
use crate::hci;
use crate::hid;
use crate::l2cap::{self, Admit, Change, Changes, Channels};
use crate::le;
use crate::scan::Scan;
use crate::sdp;

pub const CONNS: usize = 4;
/// How long a page may take before it is given up (the page timeout set is 5.12 s), and the waits after failures.
pub const PAGE_MS: u64 = 8_000;
pub const RETRY_MS: u64 = 15_000;
pub const RETRY_MAX_MS: u64 = 60_000;
/// After a device connects to us, how long we leave it to open its channels before we open ours.
pub const INCOMING_MS: u64 = 3_000;
/// The default event mask (bits 0-44) with Extended Inquiry Result (46), Encryption Key Refresh Complete (47), the
/// Secure Simple Pairing events (48-53), User Passkey Notification (58) and the LE meta event (61).
pub const EVENT_MASK: u64 = 0x0000_1FFF_FFFF_FFFF | 0xFF << 46 | 1 << 58 | 1 << 61;
/// LE: connection complete, advertising report, connection update complete, remote features, long term key request.
pub const LE_EVENT_MASK: u64 = 0x1F;
/// A desktop computer (major class 1, minor 1).
pub const CLASS_OF_DEVICE: [u8; 3] = [0x04, 0x01, 0x00];
pub const NAME: &str = "MIND Core";
/// IO capabilities: NoInputNoOutput (Just Works), and DisplayOnly for a keyboard, which then types the passkey shown.
pub const NO_INPUT_NO_OUTPUT: u8 = 0x03;
pub const DISPLAY_ONLY: u8 = 0x00;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step { Up, Authenticating, Encrypting, Secure }

/// A link: its handle, peer, transport, who made it, its security and what runs on it.
pub struct Conn {
    pub handle: u16, pub addr: Addr, pub le: bool, pub outgoing: bool, pub step: Step, pub encrypted: bool,
    pub kind: Option<Kind>, pub since: u64, pub started: bool, pub repaired: bool, pub used_key: bool, pub pdu: Pdu,
}

/// What the driver must act on: a key to keep, a HID report to deliver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Up<'a> { Keys, Keyboard(&'a [u8]), Pointer(&'a [u8]) }

pub const UP_KEYS: u8 = 1;
pub const UP_KEYBOARD: u8 = 2;
pub const UP_POINTER: u8 = 3;

pub struct Ups { pub queue: Queue<2048> }

impl Default for Ups { fn default() -> Self { Self { queue: Queue::new() } } }

impl Ups {
    pub fn push(&mut self, kind: u8, data: &[u8]) { self.queue.push(&[&[kind], data]); }
    /// The next upcall, decoded from `out`.
    pub fn take<'a>(&mut self, out: &'a mut [u8; 64]) -> Option<Up<'a>> {
        let n = self.queue.take(out)?;
        let (kind, data) = (out[0], &out[1..n]);
        Some(match kind { UP_KEYS => Up::Keys, UP_KEYBOARD => Up::Keyboard(data), _ => Up::Pointer(data) })
    }
}

pub struct Stack {
    pub ctl: Ctl,
    pub chans: Channels,
    pub conns: [Option<Conn>; CONNS],
    pub devices: Devices,
    pub scan: Scan,
    pub sdp: sdp::Server,
    pub a2dp: avdtp::Source,
    pub hid: hid::Hosts,
    pub le: le::Central,
    pub ups: Ups,
    /// The controller's features (Read Local Supported Features) and whether its buffers are known.
    pub features: [u8; 8],
    pub ready: bool,
    /// A page or an LE connection being made: the device, since when.
    pub paging: Option<(Addr, u64)>,
    pub started: bool,
}

impl Default for Stack { fn default() -> Self { Self::new() } }

fn conn_at(conns: &[Option<Conn>], handle: u16) -> Option<usize> { conns.iter().position(|c| c.as_ref().is_some_and(|c| c.handle == handle)) }

impl Stack {
    pub fn new() -> Self {
        Self { ctl: Ctl::new(), chans: Channels::new(), conns: [const { None }; CONNS], devices: Devices::new(), scan: Scan::new(), sdp: sdp::Server::new(),
               a2dp: avdtp::Source::new(), hid: hid::Hosts::new(), le: le::Central::new(), ups: Ups::default(), features: [0; 8], ready: false, paging: None, started: false }
    }

    pub fn say(&mut self, args: core::fmt::Arguments) { self.ctl.say(args) }

    pub fn conn(&self, handle: u16) -> Option<&Conn> { self.conns.iter().flatten().find(|c| c.handle == handle) }
    pub fn links(&self) -> usize { self.conns.iter().flatten().count() }
    fn le_supported(&self) -> bool { self.features[4] & 0x40 != 0 }

    /// The controller's setup for links, after bt_hci brought it up: its buffers and features read, the events the
    /// stack needs, Secure Simple Pairing, LE, its class and name; page scan (others may connect) only with devices
    /// listed, so a machine without them keeps its radio quiet.
    pub fn start(&mut self, now: u64) {
        self.ctl.now = now;
        self.started = true;
        self.ctl.command(hci::READ_LOCAL_FEATURES, &[]);
        self.ctl.command(hci::READ_BUFFER_SIZE, &[]);
        self.ctl.command(hci::SET_EVENT_MASK, &EVENT_MASK.to_le_bytes());
        self.ctl.command(hci::WRITE_SIMPLE_PAIRING_MODE, &[1]);
        self.ctl.command(hci::WRITE_INQUIRY_MODE, &[2]);
        self.ctl.command(hci::WRITE_CLASS_OF_DEVICE, &CLASS_OF_DEVICE);
        let mut name = [0u8; 248];
        name[..NAME.len()].copy_from_slice(NAME.as_bytes());
        self.ctl.command(hci::WRITE_LOCAL_NAME, &name);
        // Role switch and sniff allowed.
        self.ctl.command(hci::WRITE_DEFAULT_LINK_POLICY, &0x0005u16.to_le_bytes());
        // 0x2000 slots: 5.12 s.
        self.ctl.command(hci::WRITE_PAGE_TIMEOUT, &0x2000u16.to_le_bytes());
        if !self.devices.is_empty() { self.ctl.command(hci::WRITE_SCAN_ENABLE, &[0x02]); }
    }

    /// An event from the controller.
    pub fn event(&mut self, code: u8, p: &[u8]) {
        self.scan.event(&mut self.ctl, code, p);
        match code {
            hci::EV_COMMAND_COMPLETE if p.len() >= 3 => {
                let op = le16(p, 1);
                self.ctl.answered(p[0], op);
                self.completed(op, &p[3..]);
            }
            hci::EV_COMMAND_STATUS if p.len() >= 4 => {
                let op = le16(p, 2);
                self.ctl.answered(p[1], op);
                if p[0] != 0 { self.failed(op, p[0]); }
            }
            hci::EV_NUMBER_OF_COMPLETED_PACKETS if !p.is_empty() => {
                for i in 0..p[0] as usize { if p.len() >= 5 + 4 * i { self.ctl.completed(le16(p, 1 + 4 * i), le16(p, 3 + 4 * i)); } }
            }
            hci::EV_CONNECTION_REQUEST if p.len() >= 10 => {
                let addr = Addr::from_wire(p);
                let link = p[9];
                if link == 1 && self.devices.get(addr).is_some() && self.links() < CONNS {
                    self.say(format_args!("{} ASKS TO CONNECT: ACCEPTED", addr));
                    // We stay peripheral if the device made the link: no role switch to fail.
                    self.ctl.command(hci::ACCEPT_CONNECTION, &[p[0], p[1], p[2], p[3], p[4], p[5], 0x01]);
                } else {
                    self.say(format_args!("{} ASKS TO CONNECT (CLASS {:06X}, LINK {}): NOT A DEVICE LISTED, REFUSED", addr, u32::from_le_bytes([p[6], p[7], p[8], 0]), link));
                    // Unacceptable BD_ADDR.
                    self.ctl.command(hci::REJECT_CONNECTION, &[p[0], p[1], p[2], p[3], p[4], p[5], 0x0F]);
                }
            }
            hci::EV_CONNECTION_COMPLETE if p.len() >= 11 => {
                let (status, handle, addr, link, encrypted) = (p[0], le16(p, 1), Addr::from_wire(&p[3..]), p[9], p[10] != 0);
                let outgoing = self.paging.is_some_and(|(a, _)| a == addr);
                if outgoing { self.paging = None; }
                if status != 0 { self.page_failed(addr, status); return; }
                if link != 1 { return; }
                self.link_up(handle, addr, false, outgoing, encrypted);
            }
            hci::EV_DISCONNECTION_COMPLETE if p.len() >= 4 => {
                let (handle, reason) = (le16(p, 1), p[3]);
                self.link_down(handle, reason);
            }
            hci::EV_LINK_KEY_REQUEST if p.len() >= 6 => {
                let addr = Addr::from_wire(p);
                let key = self.devices.get(addr).and_then(|d| d.key);
                if let Some(c) = self.conns.iter_mut().flatten().find(|c| c.addr == addr) { c.used_key = key.is_some(); }
                match key {
                    Some((k, _)) => { let mut b = [0u8; 22]; b[..6].copy_from_slice(&p[..6]); b[6..].copy_from_slice(&k); self.ctl.command(hci::LINK_KEY_REPLY, &b); }
                    None => self.ctl.command(hci::LINK_KEY_NEGATIVE_REPLY, &p[..6]),
                }
            }
            hci::EV_PIN_CODE_REQUEST if p.len() >= 6 => {
                let addr = Addr::from_wire(p);
                if self.devices.get(addr).is_none() { self.ctl.command(hci::PIN_CODE_NEGATIVE_REPLY, &p[..6]); return; }
                self.say(format_args!("{}: LEGACY PAIRING, PIN 0000", addr));
                let mut b = [0u8; 23];
                b[..6].copy_from_slice(&p[..6]);
                b[6] = 4;
                b[7..11].copy_from_slice(b"0000");
                self.ctl.command(hci::PIN_CODE_REPLY, &b);
            }
            hci::EV_IO_CAPABILITY_REQUEST if p.len() >= 6 => {
                let addr = Addr::from_wire(p);
                let Some(d) = self.devices.get(addr) else {
                    // Pairing not allowed (0x18).
                    self.ctl.command(hci::IO_CAPABILITY_NEGATIVE_REPLY, &[p[0], p[1], p[2], p[3], p[4], p[5], 0x18]);
                    return;
                };
                // General bonding; MITM asked only with a display, for a keyboard.
                let (io, auth) = if d.kind == Kind::Hid { (DISPLAY_ONLY, 0x05) } else { (NO_INPUT_NO_OUTPUT, 0x04) };
                self.ctl.command(hci::IO_CAPABILITY_REPLY, &[p[0], p[1], p[2], p[3], p[4], p[5], io, 0, auth]);
            }
            hci::EV_IO_CAPABILITY_RESPONSE if p.len() >= 9 => {
                self.say(format_args!("{}: SECURE SIMPLE PAIRING, ITS IO CAPABILITY {}, AUTHENTICATION {:02X}", Addr::from_wire(p), p[6], p[8]));
            }
            hci::EV_USER_CONFIRMATION_REQUEST if p.len() >= 10 => {
                // Testing stage: every number confirmed (Just Works, or a comparison nobody here can make), and logged.
                self.say(format_args!("{}: CONFIRMED {:06} (NO ONE TO COMPARE IT: THE TESTING STAGE CONFIRMS ALL)", Addr::from_wire(p), u32::from_le_bytes([p[6], p[7], p[8], p[9]])));
                self.ctl.command(hci::USER_CONFIRMATION_REPLY, &p[..6]);
            }
            hci::EV_USER_PASSKEY_REQUEST if p.len() >= 6 => self.ctl.command(hci::USER_PASSKEY_NEGATIVE_REPLY, &p[..6]),
            hci::EV_USER_PASSKEY_NOTIFICATION if p.len() >= 10 => {
                self.say(format_args!("{}: TYPE {:06} ON IT, THEN ENTER", Addr::from_wire(p), u32::from_le_bytes([p[6], p[7], p[8], p[9]])));
            }
            hci::EV_SIMPLE_PAIRING_COMPLETE if p.len() >= 7 => {
                self.say(format_args!("{}: PAIRING {} (STATUS {:02X})", Addr::from_wire(&p[1..]), if p[0] == 0 { "DONE" } else { "FAILED" }, p[0]));
            }
            hci::EV_LINK_KEY_NOTIFICATION if p.len() >= 23 => {
                let addr = Addr::from_wire(p);
                let mut key = [0u8; 16];
                key.copy_from_slice(&p[6..22]);
                if let Some(d) = self.devices.get_mut(addr) {
                    d.key = Some((key, p[22]));
                    self.ctl.say(format_args!("{}: LINK KEY OF TYPE {} KEPT", addr, p[22]));
                    self.ups.push(UP_KEYS, &[]);
                }
            }
            hci::EV_AUTHENTICATION_COMPLETE if p.len() >= 3 => {
                let (status, handle) = (p[0], le16(p, 1));
                self.authenticated(handle, status);
            }
            hci::EV_ENCRYPTION_CHANGE if p.len() >= 4 => {
                let (status, handle, on) = (p[0], le16(p, 1), p[3] != 0);
                self.encryption(handle, status, on);
            }
            hci::EV_ENCRYPTION_KEY_REFRESH if p.len() >= 3 => {}
            hci::EV_HARDWARE_ERROR => self.say(format_args!("CONTROLLER HARDWARE ERROR {:02X}", p.first().copied().unwrap_or(0))),
            hci::EV_DATA_BUFFER_OVERFLOW => self.say(format_args!("THE CONTROLLER'S ACL BUFFERS OVERFLOWED")),
            hci::EV_LE_META if !p.is_empty() => self.le_event(p[0], &p[1..]),
            _ => {}
        }
    }

    fn completed(&mut self, op: u16, r: &[u8]) {
        let status = r.first().copied().unwrap_or(0xFF);
        if status != 0 { self.failed(op, status); return; }
        match op {
            hci::READ_BUFFER_SIZE if r.len() >= 8 => {
                let (len, total) = (le16(r, 1), le16(r, 4));
                self.ctl.bredr = crate::ctl::Pool { len, total, free: total };
                self.say(format_args!("BR/EDR BUFFERS: {} OF {} BYTES", total, len));
                if self.le_supported() { self.ctl.command(hci::LE_READ_BUFFER_SIZE, &[]); }
                self.ready = true;
            }
            hci::LE_READ_BUFFER_SIZE if r.len() >= 4 => {
                let (len, total) = (le16(r, 1), r[3] as u16);
                self.ctl.le = crate::ctl::Pool { len, total, free: total };
                if total == 0 { self.say(format_args!("LE BUFFERS: SHARED WITH BR/EDR")); } else { self.say(format_args!("LE BUFFERS: {} OF {} BYTES", total, len)); }
            }
            hci::READ_LOCAL_FEATURES if r.len() >= 9 => {
                self.features.copy_from_slice(&r[1..9]);
                let f = self.features;
                self.say(format_args!("FEATURES {:02X?}: SECURE SIMPLE PAIRING {}, LE {}", f, f[6] & 0x08 != 0, f[4] & 0x40 != 0));
                if self.le_supported() {
                    self.ctl.command(hci::LE_SET_EVENT_MASK, &LE_EVENT_MASK.to_le_bytes());
                    self.ctl.command(hci::WRITE_LE_HOST_SUPPORTED, &[1, 0]);
                }
            }
            _ => {}
        }
    }

    /// A command refused (its Command Status or Command Complete status).
    fn failed(&mut self, op: u16, status: u8) {
        self.say(format_args!("COMMAND {:04X} FAILED: STATUS {:02X}", op, status));
        match op {
            hci::CREATE_CONNECTION | hci::LE_CREATE_CONNECTION => if let Some((addr, _)) = self.paging.take() { self.page_failed(addr, status); },
            hci::AUTHENTICATION_REQUESTED | hci::SET_CONNECTION_ENCRYPTION => {
                // The link it was for cannot be secured: closed.
                if let Some(c) = self.conns.iter().flatten().find(|c| matches!(c.step, Step::Authenticating | Step::Encrypting)) { let h = c.handle; self.disconnect(h); }
            }
            _ => {}
        }
    }

    fn page_failed(&mut self, addr: Addr, status: u8) {
        let Some(d) = self.devices.get_mut(addr) else { return };
        d.failures += 1;
        let wait = (RETRY_MS << (d.failures - 1).min(2)).min(RETRY_MAX_MS);
        d.next_try = self.ctl.now + wait;
        // 04: page timeout, the device not there or not answering.
        self.ctl.say(format_args!("{}: NOT CONNECTED (STATUS {:02X}), TRIED AGAIN IN {} S", addr, status, wait / 1000));
    }

    fn link_up(&mut self, handle: u16, addr: Addr, le: bool, outgoing: bool, encrypted: bool) {
        let Some(slot) = self.conns.iter().position(Option::is_none) else { self.ctl.command(hci::DISCONNECT, &[handle as u8, (handle >> 8) as u8, 0x13]); return };
        let kind = self.devices.get(addr).map(|d| d.kind);
        if let Some(d) = self.devices.get_mut(addr) { d.failures = 0; }
        self.ctl.link_up(handle, le);
        self.conns[slot] = Some(Conn { handle, addr, le, outgoing, step: Step::Up, encrypted, kind, since: self.ctl.now, started: false, repaired: false, used_key: false, pdu: Pdu::new() });
        self.say(format_args!("{} CONNECTED{}: HANDLE {:03X}, {}", addr, if le { " OVER LE" } else { "" }, handle, if outgoing { "WE PAGED IT" } else { "IT CAME TO US" }));
        if le { self.le.link_up(&mut self.ctl, handle, addr, self.devices.get(addr).and_then(|d| d.ltk)); return; }
        if outgoing { self.secure(handle); }
    }

    fn link_down(&mut self, handle: u16, reason: u8) {
        let Some(i) = conn_at(&self.conns, handle) else { return };
        let c = self.conns[i].take().unwrap();
        self.ctl.link_down(handle);
        self.say(format_args!("{} DISCONNECTED (REASON {:02X})", c.addr, reason));
        let mut changes = Changes::default();
        self.chans.link_down(handle, &mut changes);
        self.apply(&changes);
        self.a2dp.link_down(handle);
        self.hid.link_down(handle, &mut self.ups);
        if c.le { self.le.link_down(handle); }
        if let Some(d) = self.devices.get_mut(c.addr) { d.next_try = self.ctl.now + 5_000; }
    }

    /// Authentication then encryption for a BR/EDR link, unless it is secure already.
    fn secure(&mut self, handle: u16) {
        let Some(c) = self.conns.iter_mut().flatten().find(|c| c.handle == handle) else { return };
        if c.step != Step::Up { return; }
        c.step = Step::Authenticating;
        self.ctl.command(hci::AUTHENTICATION_REQUESTED, &handle.to_le_bytes());
    }

    fn authenticated(&mut self, handle: u16, status: u8) {
        let Some(i) = conn_at(&self.conns, handle) else { return };
        let c = self.conns[i].as_mut().unwrap();
        let addr = c.addr;
        if status == 0 {
            if c.encrypted { c.step = Step::Secure; self.ready_link(handle); return; }
            c.step = Step::Encrypting;
            self.ctl.command(hci::SET_CONNECTION_ENCRYPTION, &[handle as u8, (handle >> 8) as u8, 1]);
            return;
        }
        // A key the device no longer has (PIN or key missing 06, authentication failure 05): forgotten, paired again once.
        if matches!(status, 0x05 | 0x06) && c.used_key && !c.repaired {
            c.repaired = true;
            c.step = Step::Authenticating;
            if let Some(d) = self.devices.get_mut(addr) { d.key = None; }
            self.ctl.say(format_args!("{}: ITS KEY REFUSED (STATUS {:02X}): FORGOTTEN, PAIRING AGAIN", addr, status));
            self.ups.push(UP_KEYS, &[]);
            self.ctl.command(hci::AUTHENTICATION_REQUESTED, &handle.to_le_bytes());
            return;
        }
        c.step = Step::Up;
        self.ctl.say(format_args!("{}: AUTHENTICATION FAILED (STATUS {:02X}): DISCONNECTED", addr, status));
        self.disconnect(handle);
    }

    fn encryption(&mut self, handle: u16, status: u8, on: bool) {
        let Some(i) = conn_at(&self.conns, handle) else { return };
        let c = self.conns[i].as_mut().unwrap();
        c.encrypted = status == 0 && on;
        let (addr, le) = (c.addr, c.le);
        if le { self.le.encrypted(&mut self.ctl, handle, status, on); return; }
        if !c.encrypted {
            self.ctl.say(format_args!("{}: ENCRYPTION {} (STATUS {:02X}): DISCONNECTED", addr, if on { "ON" } else { "OFF" }, status));
            self.disconnect(handle);
            return;
        }
        self.ctl.say(format_args!("{}: ENCRYPTED", addr));
        if c.step != Step::Secure { c.step = Step::Secure; self.ready_link(handle); }
    }

    /// A secure BR/EDR link: channels held for security admitted, then our profile started on a link we made; a device
    /// that came to us is left INCOMING_MS to open its own.
    fn ready_link(&mut self, handle: u16) {
        let mut changes = Changes::default();
        let waiting: [Option<u16>; l2cap::CHANNELS] = core::array::from_fn(|i| self.chans.list[i].filter(|c| c.handle == handle && c.state == l2cap::State::Pending).map(|c| c.local));
        for local in waiting.into_iter().flatten() { self.chans.admit(&mut self.ctl, handle, local, true, &mut changes); }
        self.apply(&changes);
        let now = self.ctl.now;
        let Some(c) = self.conns.iter_mut().flatten().find(|c| c.handle == handle) else { return };
        if c.outgoing { self.start_profile(handle); } else { c.since = now; }
    }

    fn start_profile(&mut self, handle: u16) {
        let Some(c) = self.conns.iter_mut().flatten().find(|c| c.handle == handle) else { return };
        if c.started || c.step != Step::Secure { return; }
        c.started = true;
        match c.kind {
            Some(Kind::Audio) => self.a2dp.start(&mut self.ctl, &mut self.chans, handle),
            Some(Kind::Hid) => self.hid.start(&mut self.ctl, &mut self.chans, handle),
            _ => {}
        }
    }

    pub fn disconnect(&mut self, handle: u16) { self.ctl.command(hci::DISCONNECT, &[handle as u8, (handle >> 8) as u8, 0x13]); }

    /// One ACL packet from the controller.
    pub fn acl(&mut self, packet: &[u8]) {
        let Some((handle, pb, data)) = acl::parse(packet) else { return };
        let Some(i) = conn_at(&self.conns, handle) else { return };
        let mut pdu = [0u8; acl::PDU_MAX];
        let n = {
            let Some(whole) = self.conns[i].as_mut().unwrap().pdu.feed(pb != acl::PB_CONTINUING, data) else { return };
            pdu[..whole.len()].copy_from_slice(whole);
            whole.len()
        };
        let (cid, payload) = (le16(&pdu, 2), &pdu[4..n]);
        self.l2cap(i, handle, cid, payload);
    }

    fn l2cap(&mut self, i: usize, handle: u16, cid: u16, payload: &[u8]) {
        let c = self.conns[i].as_ref().unwrap();
        let (le, secure, kind) = (c.le, c.step == Step::Secure, c.kind);
        match cid {
            l2cap::SIGNALLING if !le => {
                let mut changes = Changes::default();
                let admit = |psm: u16| match (psm, kind) {
                    (l2cap::PSM_SDP, _) => Admit::Accept,
                    (l2cap::PSM_AVDTP, Some(Kind::Audio)) | (l2cap::PSM_HID_CONTROL | l2cap::PSM_HID_INTERRUPT, Some(Kind::Hid)) => if secure { Admit::Accept } else { Admit::Pending },
                    _ => Admit::Refuse,
                };
                self.chans.signal(&mut self.ctl, handle, payload, admit, &mut changes);
                self.apply(&changes);
            }
            l2cap::LE_SIGNALLING if le => {
                if let Some(p) = l2cap::le_signal(&mut self.ctl, handle, payload) {
                    let mut b = [0u8; 14];
                    b[..2].copy_from_slice(&handle.to_le_bytes());
                    for (k, v) in [p.min, p.max, p.latency, p.timeout].iter().enumerate() { b[2 + 2 * k..4 + 2 * k].copy_from_slice(&v.to_le_bytes()); }
                    self.ctl.command(hci::LE_CONNECTION_UPDATE, &b);
                }
            }
            l2cap::ATT | l2cap::SMP if le => self.le.pdu(&mut self.ctl, &mut self.ups, &mut self.devices, handle, cid, payload),
            _ => {
                let Some(ch) = self.chans.get(cid).filter(|ch| ch.handle == handle && ch.state == l2cap::State::Open).copied() else { return };
                match ch.psm {
                    l2cap::PSM_SDP => self.sdp.pdu(&mut self.ctl, &self.chans, handle, cid, payload),
                    l2cap::PSM_AVDTP => self.a2dp.data(&mut self.ctl, &mut self.chans, handle, cid, payload),
                    l2cap::PSM_HID_CONTROL | l2cap::PSM_HID_INTERRUPT => self.hid.data(&mut self.ctl, &mut self.chans, &mut self.ups, handle, cid, payload),
                    _ => {}
                }
            }
        }
    }

    /// Channel changes told to their profiles; a channel waiting for security starts it.
    fn apply(&mut self, changes: &Changes) {
        for change in changes.iter() {
            match change {
                Change::Opened { handle, local, psm, initiator } => {
                    // The device opened a profile's channel: it runs the profile, we open none of our own.
                    if !initiator && psm != l2cap::PSM_SDP { if let Some(c) = self.conns.iter_mut().flatten().find(|c| c.handle == handle) { c.started = true; } }
                    self.say(format_args!("L2CAP CHANNEL {:04X} TO PSM {:04X} OPEN ({}), MTU {}", local, psm, if initiator { "OURS" } else { "THEIRS" }, self.chans.get(local).map_or(0, |c| c.mtu)));
                    match psm {
                        l2cap::PSM_AVDTP => self.a2dp.opened(&mut self.ctl, &mut self.chans, handle, local, initiator),
                        l2cap::PSM_HID_CONTROL | l2cap::PSM_HID_INTERRUPT => self.hid.opened(&mut self.ctl, &mut self.chans, handle, local, psm, initiator),
                        _ => {}
                    }
                }
                Change::Closed { handle, local, psm } => {
                    self.say(format_args!("L2CAP CHANNEL {:04X} TO PSM {:04X} CLOSED", local, psm));
                    match psm {
                        l2cap::PSM_AVDTP => self.a2dp.closed(&mut self.ctl, &mut self.chans, handle, local),
                        l2cap::PSM_HID_CONTROL | l2cap::PSM_HID_INTERRUPT => self.hid.closed(&mut self.ctl, &mut self.chans, &mut self.ups, handle, local),
                        _ => {}
                    }
                }
                Change::Refused { handle, psm, result } => {
                    self.say(format_args!("L2CAP CHANNEL TO PSM {:04X} NOT OPENED (RESULT {})", psm, result));
                    match psm {
                        l2cap::PSM_AVDTP => self.a2dp.refused(handle),
                        l2cap::PSM_HID_CONTROL | l2cap::PSM_HID_INTERRUPT => self.hid.refused(&mut self.ctl, &mut self.chans, handle),
                        _ => {}
                    }
                }
                Change::Waiting { handle, .. } => {
                    let secure = self.conn(handle).is_some_and(|c| c.step == Step::Secure);
                    if secure { self.ready_link(handle) } else {
                        // The device asks before securing the link itself: it stays the initiator of its profile.
                        if let Some(c) = self.conns.iter_mut().flatten().find(|c| c.handle == handle) { c.started = true; }
                        self.secure(handle);
                    }
                }
            }
        }
    }

    fn le_event(&mut self, sub: u8, p: &[u8]) {
        match sub {
            hci::LE_CONNECTION_COMPLETE | hci::LE_ENHANCED_CONNECTION_COMPLETE if p.len() >= 11 => {
                let (status, handle, role, addr) = (p[0], le16(p, 1), p[3], Addr::from_wire(&p[5..]));
                let outgoing = self.paging.is_some_and(|(a, _)| a == addr);
                if outgoing || status != 0 { self.paging = None; }
                if status != 0 { if let Some(d) = self.devices.list.iter().flatten().find(|d| d.kind == Kind::LeHid) { let a = d.addr; self.page_failed(a, status); } return; }
                if role != 0 { self.disconnect(handle); return; }
                self.link_up(handle, addr, true, true, false);
            }
            hci::LE_CONNECTION_UPDATE_COMPLETE if p.len() >= 9 => {
                self.say(format_args!("LE {:03X}: INTERVAL {} × 1.25 MS, LATENCY {}, TIMEOUT {} × 10 MS (STATUS {:02X})", le16(p, 1), le16(p, 3), le16(p, 5), le16(p, 7), p[0]));
            }
            _ => {}
        }
    }

    /// The time: pages and LE connections started for the devices listed, timeouts, the profiles' own work.
    pub fn tick(&mut self, now: u64) {
        self.ctl.now = now;
        let mut changes = Changes::default();
        self.chans.tick(&mut self.ctl, &mut changes);
        self.apply(&changes);
        if !self.ready { return; }
        let busy = self.links() > 0 || self.paging.is_some();
        self.scan.tick(&mut self.ctl, busy);
        // A page that never ended: cancelled.
        if let Some((addr, since)) = self.paging {
            if now > since + PAGE_MS + 4_000 {
                self.ctl.say(format_args!("{}: NO ANSWER TO THE PAGE: CANCELLED", addr));
                let le = self.devices.get(addr).is_some_and(|d| d.kind == Kind::LeHid);
                if le { self.ctl.command(hci::LE_CREATE_CONNECTION_CANCEL, &[]); } else { self.ctl.command(hci::CREATE_CONNECTION_CANCEL, &addr.0); }
                self.paging = None;
                self.page_failed(addr, 0x04);
            }
        }
        // A device that came to us and opened nothing: we secure the link, then run our profile.
        let late: [Option<(u16, Step)>; CONNS] = core::array::from_fn(|i| self.conns[i].as_ref()
            .filter(|c| !c.le && !c.outgoing && !c.started && matches!(c.step, Step::Up | Step::Secure) && now > c.since + INCOMING_MS).map(|c| (c.handle, c.step)));
        for (h, step) in late.into_iter().flatten() { if step == Step::Up { self.secure(h) } else { self.start_profile(h) } }
        if self.paging.is_none() && !self.scan.active() && self.ctl.commands_queued() == 0 {
            let due = self.devices.list.iter().flatten().find(|d| now >= d.next_try && !self.conns.iter().flatten().any(|c| c.addr == d.addr)).copied();
            if let Some(d) = due {
                self.paging = Some((d.addr, now));
                if let Some(dev) = self.devices.get_mut(d.addr) { dev.next_try = now + RETRY_MS; }
                if d.kind == Kind::LeHid {
                    self.say(format_args!("{}: CONNECTING OVER LE ({} ADDRESS)", d.addr, if d.random { "RANDOM" } else { "PUBLIC" }));
                    self.ctl.command(hci::LE_CREATE_CONNECTION, &le::create_connection(d.addr, d.random));
                } else {
                    self.say(format_args!("{}: PAGING ({})", d.addr, d.kind.name()));
                    let mut b = [0u8; 13];
                    b[..6].copy_from_slice(&d.addr.0);
                    // DM1 to DH5, page scan repetition R1, no clock offset, role switch allowed.
                    b[6..8].copy_from_slice(&0xCC18u16.to_le_bytes());
                    b[8] = 1;
                    b[12] = 1;
                    self.ctl.command(hci::CREATE_CONNECTION, &b);
                }
            }
        }
        self.a2dp.tick(&mut self.ctl, &mut self.chans);
        self.hid.tick(&mut self.ctl, &mut self.chans);
        self.le.tick(&mut self.ctl);
    }

    /// The LE long term key a link's pairing gave, kept for the device.
    pub fn keep_ltk(&mut self, addr: Addr, ltk: Ltk) { if let Some(d) = self.devices.get_mut(addr) { d.ltk = Some(ltk); self.ups.push(UP_KEYS, &[]); } }
}
