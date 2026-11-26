//! L2CAP (Core 5.3 Vol 3 Part A) in basic mode: the BR/EDR signalling channel's connection-oriented channels (connect,
//! configure the MTU, disconnect, information, echo) both as initiator and as responder, and the LE signalling
//! channel's connection parameter update. Segmentation and reassembly are acl.rs's; enhanced retransmission, streaming
//! and credit-based modes are refused, as basic mode is all the profiles here need.
use crate::buf::le16;
use crate::ctl::Ctl;

pub const SIGNALLING: u16 = 0x0001;
pub const ATT: u16 = 0x0004;
pub const LE_SIGNALLING: u16 = 0x0005;
pub const SMP: u16 = 0x0006;
pub const FIRST_DYNAMIC: u16 = 0x0040;

pub const PSM_SDP: u16 = 0x0001;
pub const PSM_HID_CONTROL: u16 = 0x0011;
pub const PSM_HID_INTERRUPT: u16 = 0x0013;
pub const PSM_AVDTP: u16 = 0x0019;
pub const PSM_AVCTP: u16 = 0x0017;

pub const COMMAND_REJECT: u8 = 0x01;
pub const CONNECTION_REQUEST: u8 = 0x02;
pub const CONNECTION_RESPONSE: u8 = 0x03;
pub const CONFIGURATION_REQUEST: u8 = 0x04;
pub const CONFIGURATION_RESPONSE: u8 = 0x05;
pub const DISCONNECTION_REQUEST: u8 = 0x06;
pub const DISCONNECTION_RESPONSE: u8 = 0x07;
pub const ECHO_REQUEST: u8 = 0x08;
pub const ECHO_RESPONSE: u8 = 0x09;
pub const INFORMATION_REQUEST: u8 = 0x0A;
pub const INFORMATION_RESPONSE: u8 = 0x0B;
pub const PARAMETER_UPDATE_REQUEST: u8 = 0x12;
pub const PARAMETER_UPDATE_RESPONSE: u8 = 0x13;

/// Connection Response results.
pub const SUCCESS: u16 = 0;
pub const PENDING: u16 = 1;
pub const PSM_NOT_SUPPORTED: u16 = 2;
pub const SECURITY_BLOCK: u16 = 3;
pub const NO_RESOURCES: u16 = 4;

/// The MTU bt_host takes on every channel; the default when a peer names none.
pub const MTU: u16 = 1021;
pub const DEFAULT_MTU: u16 = 672;
pub const CHANNELS: usize = 16;
/// How long a channel may take to connect and be configured.
pub const SETUP_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Connecting, Pending, Config, Open, Closing }

/// A connection-oriented channel: its link, our and the peer's channel IDs, its PSM, its state, the MTU the peer
/// takes, whether each side's configuration is done, who opened it, the identifier of the request answered or awaited.
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub handle: u16, pub local: u16, pub remote: u16, pub psm: u16, pub state: State, pub mtu: u16,
    pub ours: bool, pub theirs: bool, pub initiator: bool, pub ident: u8, pub since: u64,
}

/// What a signalling exchange did to the channels, for the stack to tell their profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Opened { handle: u16, local: u16, psm: u16, initiator: bool },
    Closed { handle: u16, local: u16, psm: u16 },
    Refused { handle: u16, psm: u16, result: u16 },
    /// A peer asks for a channel to a PSM that waits for the link's security: the stack admits or refuses it.
    Waiting { handle: u16, local: u16, psm: u16 },
}

/// A responder's answer to a peer's Connection Request for a PSM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit { Accept, Pending, Refuse }

/// Changes collected while a PDU is handled.
pub struct Changes { pub list: [Option<Change>; 8] }

impl Default for Changes { fn default() -> Self { Self { list: [None; 8] } } }

impl Changes {
    pub fn push(&mut self, c: Change) { if let Some(slot) = self.list.iter_mut().find(|s| s.is_none()) { *slot = Some(c); } }
    pub fn iter(&self) -> impl Iterator<Item = Change> + '_ { self.list.iter().flatten().copied() }
}

pub struct Channels { pub list: [Option<Channel>; CHANNELS], next: u16, ident: u8 }

impl Default for Channels { fn default() -> Self { Self::new() } }

fn command(ctl: &mut Ctl, handle: u16, cid: u16, code: u8, ident: u8, data: &[&[u8]]) {
    let len: usize = data.iter().map(|d| d.len()).sum();
    let head = [code, ident, len as u8, (len >> 8) as u8];
    let mut parts: [&[u8]; 5] = [&head, &[], &[], &[], &[]];
    for (i, d) in data.iter().take(4).enumerate() { parts[1 + i] = d; }
    if !ctl.l2cap(handle, cid, &parts[..1 + data.len().min(4)]) { ctl.say(format_args!("L2CAP SIGNAL {:02X} ON {:04X} NOT QUEUED", code, handle)); }
}

fn w(v: u16) -> [u8; 2] { v.to_le_bytes() }

impl Channels {
    pub const fn new() -> Self { Self { list: [None; CHANNELS], next: FIRST_DYNAMIC, ident: 0 } }

    fn ident(&mut self) -> u8 { self.ident = if self.ident == 0xFF { 1 } else { self.ident + 1 }; self.ident }

    fn free_cid(&mut self) -> u16 {
        loop {
            let cid = self.next;
            self.next = if self.next == 0xFFFF { FIRST_DYNAMIC } else { self.next + 1 };
            if !self.list.iter().flatten().any(|c| c.local == cid) { return cid; }
        }
    }

    pub fn get(&self, local: u16) -> Option<&Channel> { self.list.iter().flatten().find(|c| c.local == local) }
    fn at(&self, handle: u16, local: u16) -> Option<usize> { self.list.iter().position(|c| c.is_some_and(|c| c.handle == handle && c.local == local)) }
    /// The open or opening channel of `handle` to `psm`, the first if several.
    pub fn find(&self, handle: u16, psm: u16) -> Option<&Channel> { self.list.iter().flatten().find(|c| c.handle == handle && c.psm == psm) }
    pub fn count(&self, handle: u16, psm: u16) -> usize { self.list.iter().flatten().filter(|c| c.handle == handle && c.psm == psm).count() }

    /// Opens a channel to `psm` over `handle`; our channel ID, or None when the table is full.
    pub fn connect(&mut self, ctl: &mut Ctl, handle: u16, psm: u16) -> Option<u16> {
        let slot = self.list.iter().position(Option::is_none)?;
        let (local, ident) = (self.free_cid(), self.ident());
        self.list[slot] = Some(Channel { handle, local, remote: 0, psm, state: State::Connecting, mtu: DEFAULT_MTU, ours: false, theirs: false, initiator: true, ident, since: ctl.now });
        command(ctl, handle, SIGNALLING, CONNECTION_REQUEST, ident, &[&w(psm), &w(local)]);
        Some(local)
    }

    fn configure(&mut self, ctl: &mut Ctl, i: usize, mtu: u16) {
        let ident = self.ident();
        let Some(c) = self.list[i].as_mut() else { return };
        (c.state, c.ident) = (State::Config, ident);
        let (handle, remote) = (c.handle, c.remote);
        command(ctl, handle, SIGNALLING, CONFIGURATION_REQUEST, ident, &[&w(remote), &w(0), &[0x01, 2], &w(mtu)]);
    }

    /// Answers a channel the stack held pending: accepted (configuration starts) or refused for security.
    pub fn admit(&mut self, ctl: &mut Ctl, handle: u16, local: u16, accept: bool, changes: &mut Changes) {
        let Some(i) = self.at(handle, local) else { return };
        let Some(c) = self.list[i] else { return };
        if c.state != State::Pending { return; }
        let result = if accept { SUCCESS } else { SECURITY_BLOCK };
        command(ctl, handle, SIGNALLING, CONNECTION_RESPONSE, c.ident, &[&w(if accept { local } else { 0 }), &w(c.remote), &w(result), &w(0)]);
        if accept { self.configure(ctl, i, MTU); } else { self.list[i] = None; changes.push(Change::Refused { handle, psm: c.psm, result }); }
    }

    /// Closes a channel: a Disconnection Request, the channel gone once answered.
    pub fn disconnect(&mut self, ctl: &mut Ctl, handle: u16, local: u16) {
        let Some(i) = self.at(handle, local) else { return };
        let ident = self.ident();
        let Some(c) = self.list[i].as_mut() else { return };
        if c.remote == 0 { self.list[i] = None; return; }
        (c.state, c.ident, c.since) = (State::Closing, ident, ctl.now);
        let remote = c.remote;
        command(ctl, handle, SIGNALLING, DISCONNECTION_REQUEST, ident, &[&w(remote), &w(local)]);
    }

    /// Sends one SDU on an open channel; false when it is not open, longer than the peer's MTU, or not queued.
    pub fn send(&self, ctl: &mut Ctl, handle: u16, local: u16, parts: &[&[u8]]) -> bool {
        let Some(c) = self.list.iter().flatten().find(|c| c.handle == handle && c.local == local && c.state == State::Open) else { return false };
        let len: usize = parts.iter().map(|p| p.len()).sum();
        if len > c.mtu as usize { ctl.say(format_args!("L2CAP SDU OF {} BYTES ON {:04X}: OVER THE PEER'S MTU {}", len, local, c.mtu)); return false; }
        ctl.l2cap(handle, c.remote, parts)
    }

    /// The link went down: its channels are gone.
    pub fn link_down(&mut self, handle: u16, changes: &mut Changes) {
        for slot in self.list.iter_mut() {
            if let Some(c) = slot.filter(|c| c.handle == handle) {
                if c.state == State::Open { changes.push(Change::Closed { handle, local: c.local, psm: c.psm }); }
                else if c.initiator && c.state != State::Closing { changes.push(Change::Refused { handle, psm: c.psm, result: NO_RESOURCES }); }
                *slot = None;
            }
        }
    }

    /// Channels that took too long to set up or to close are dropped.
    pub fn tick(&mut self, ctl: &mut Ctl, changes: &mut Changes) {
        for slot in self.list.iter_mut() {
            let Some(c) = *slot else { continue };
            if c.state == State::Open || ctl.now < c.since + SETUP_MS { continue; }
            ctl.say(format_args!("L2CAP CHANNEL {:04X} TO PSM {:04X}: NO ANSWER IN {} S ({:?})", c.local, c.psm, SETUP_MS / 1000, c.state));
            if c.initiator && c.state != State::Closing { changes.push(Change::Refused { handle: c.handle, psm: c.psm, result: NO_RESOURCES }); }
            *slot = None;
        }
    }

    /// The BR/EDR signalling channel's PDU payload from `handle`: every command in it. `admit` says whether a PSM a
    /// peer asks for is served.
    pub fn signal(&mut self, ctl: &mut Ctl, handle: u16, mut data: &[u8], mut admit: impl FnMut(u16) -> Admit, changes: &mut Changes) {
        while data.len() >= 4 {
            let (code, ident, len) = (data[0], data[1], le16(data, 2) as usize);
            let Some(body) = data.get(4..4 + len) else { break };
            self.one(ctl, handle, code, ident, body, &mut admit, changes);
            data = &data[4 + len..];
        }
    }

    fn one(&mut self, ctl: &mut Ctl, handle: u16, code: u8, ident: u8, b: &[u8], admit: &mut impl FnMut(u16) -> Admit, changes: &mut Changes) {
        match code {
            CONNECTION_REQUEST if b.len() >= 4 => {
                let (psm, remote) = (le16(b, 0), le16(b, 2));
                let answer = admit(psm);
                let slot = self.list.iter().position(Option::is_none);
                let (Some(slot), Admit::Accept | Admit::Pending) = (slot, answer) else {
                    let result = if answer == Admit::Refuse { PSM_NOT_SUPPORTED } else { NO_RESOURCES };
                    ctl.say(format_args!("L2CAP: PSM {:04X} REFUSED TO {:03X} (RESULT {})", psm, handle, result));
                    command(ctl, handle, SIGNALLING, CONNECTION_RESPONSE, ident, &[&w(0), &w(remote), &w(result), &w(0)]);
                    return;
                };
                let local = self.free_cid();
                let state = if answer == Admit::Pending { State::Pending } else { State::Config };
                self.list[slot] = Some(Channel { handle, local, remote, psm, state, mtu: DEFAULT_MTU, ours: false, theirs: false, initiator: false, ident, since: ctl.now });
                if answer == Admit::Pending {
                    // Authentication pending (status 1), until the stack admits it.
                    command(ctl, handle, SIGNALLING, CONNECTION_RESPONSE, ident, &[&w(local), &w(remote), &w(PENDING), &w(1)]);
                    changes.push(Change::Waiting { handle, local, psm });
                } else {
                    command(ctl, handle, SIGNALLING, CONNECTION_RESPONSE, ident, &[&w(local), &w(remote), &w(SUCCESS), &w(0)]);
                    self.configure(ctl, slot, MTU);
                }
            }
            CONNECTION_RESPONSE if b.len() >= 8 => {
                let (remote, local, result) = (le16(b, 0), le16(b, 2), le16(b, 4));
                let Some(i) = self.at(handle, local).filter(|&i| self.list[i].is_some_and(|c| c.state == State::Connecting)) else { return };
                let c = self.list[i].unwrap();
                match result {
                    SUCCESS => { if let Some(c) = self.list[i].as_mut() { c.remote = remote; } self.configure(ctl, i, MTU); }
                    PENDING => {}
                    _ => {
                        ctl.say(format_args!("L2CAP: {:03X} REFUSED PSM {:04X} (RESULT {})", handle, c.psm, result));
                        self.list[i] = None;
                        changes.push(Change::Refused { handle, psm: c.psm, result });
                    }
                }
            }
            CONFIGURATION_REQUEST if b.len() >= 4 => {
                let (local, flags) = (le16(b, 0), le16(b, 2));
                let Some(i) = self.at(handle, local).filter(|&i| self.list[i].is_some_and(|c| matches!(c.state, State::Config | State::Open))) else {
                    // Invalid CID in request (reason 2), with the CIDs named.
                    command(ctl, handle, SIGNALLING, COMMAND_REJECT, ident, &[&w(2), &w(local), &w(0)]);
                    return;
                };
                let c = self.list[i].unwrap();
                let (mut result, mut mtu) = (SUCCESS, c.mtu);
                let mut answer = [0u8; 64]; let mut n = 0;
                let mut options = &b[4..];
                while options.len() >= 2 {
                    let (kind, len) = (options[0], options[1] as usize);
                    let Some(value) = options.get(2..2 + len) else { break };
                    match kind & 0x7F {
                        0x01 if len == 2 => mtu = le16(value, 0).max(48),
                        // Flush timeout and quality of service: taken as they are.
                        0x02 | 0x03 => {}
                        // Retransmission and flow control: basic mode only.
                        0x04 if len >= 1 && value[0] != 0 => {
                            if result == SUCCESS { result = 1; n = 0; }
                            if result == 1 && n + 11 <= answer.len() { answer[n..n + 11].copy_from_slice(&[0x04, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0]); n += 11; }
                        }
                        0x04 | 0x05 => {}
                        _ if kind & 0x80 != 0 => {}
                        _ => {
                            if result != 3 { result = 3; n = 0; }
                            if n + 2 + len <= answer.len() { answer[n..n + 2 + len].copy_from_slice(&options[..2 + len]); n += 2 + len; }
                        }
                    }
                    options = &options[2 + len..];
                }
                let continued = flags & 1;
                command(ctl, handle, SIGNALLING, CONFIGURATION_RESPONSE, ident, &[&w(c.remote), &w(continued), &w(result), &answer[..n]]);
                if result == SUCCESS {
                    if let Some(c) = self.list[i].as_mut() { c.mtu = mtu; c.theirs = continued == 0; }
                    self.opened(i, changes);
                }
            }
            CONFIGURATION_RESPONSE if b.len() >= 6 => {
                let (local, result) = (le16(b, 0), le16(b, 4));
                let Some(i) = self.at(handle, local).filter(|&i| self.list[i].is_some_and(|c| c.state == State::Config && c.ident == ident)) else { return };
                match result {
                    SUCCESS => { if let Some(c) = self.list[i].as_mut() { c.ours = true; } self.opened(i, changes); }
                    4 => {}
                    1 => {
                        // Unacceptable: the MTU they name instead, once.
                        let suggested = b.get(6..).and_then(|o| (o.len() >= 4 && o[0] & 0x7F == 1 && o[1] == 2).then(|| le16(o, 2)));
                        match suggested { Some(m) if m != MTU && m >= 48 => self.configure(ctl, i, m), _ => { let c = self.list[i].unwrap(); self.disconnect(ctl, handle, c.local); } }
                    }
                    _ => {
                        let c = self.list[i].unwrap();
                        ctl.say(format_args!("L2CAP: {:03X} REJECTED THE CONFIGURATION OF {:04X} (RESULT {})", handle, c.local, result));
                        self.disconnect(ctl, handle, c.local);
                    }
                }
            }
            DISCONNECTION_REQUEST if b.len() >= 4 => {
                let (local, remote) = (le16(b, 0), le16(b, 2));
                command(ctl, handle, SIGNALLING, DISCONNECTION_RESPONSE, ident, &[&w(local), &w(remote)]);
                if let Some(i) = self.at(handle, local) {
                    let c = self.list[i].unwrap();
                    self.list[i] = None;
                    if c.state == State::Open { changes.push(Change::Closed { handle, local, psm: c.psm }); }
                    else if c.initiator { changes.push(Change::Refused { handle, psm: c.psm, result: NO_RESOURCES }); }
                }
            }
            DISCONNECTION_RESPONSE if b.len() >= 4 => {
                let local = le16(b, 2);
                if let Some(i) = self.at(handle, local) {
                    let c = self.list[i].unwrap();
                    self.list[i] = None;
                    changes.push(Change::Closed { handle, local, psm: c.psm });
                }
            }
            ECHO_REQUEST => command(ctl, handle, SIGNALLING, ECHO_RESPONSE, ident, &[b]),
            // Basic mode only: no extended features, so no fixed channels to name.
            INFORMATION_REQUEST if b.len() >= 2 => match le16(b, 0) {
                1 => command(ctl, handle, SIGNALLING, INFORMATION_RESPONSE, ident, &[&w(1), &w(0), &w(MTU)]),
                2 => command(ctl, handle, SIGNALLING, INFORMATION_RESPONSE, ident, &[&w(2), &w(0), &[0, 0, 0, 0]]),
                kind => command(ctl, handle, SIGNALLING, INFORMATION_RESPONSE, ident, &[&w(kind), &w(1)]),
            },
            ECHO_RESPONSE | INFORMATION_RESPONSE => {}
            COMMAND_REJECT => {
                // Our request with that identifier failed.
                if let Some(i) = self.list.iter().position(|c| c.is_some_and(|c| c.handle == handle && c.ident == ident && c.state != State::Open)) {
                    let c = self.list[i].unwrap();
                    self.list[i] = None;
                    if c.initiator && c.state != State::Closing { changes.push(Change::Refused { handle, psm: c.psm, result: NO_RESOURCES }); }
                    if c.state == State::Closing { changes.push(Change::Closed { handle, local: c.local, psm: c.psm }); }
                }
            }
            // Command not understood (reason 0).
            _ => command(ctl, handle, SIGNALLING, COMMAND_REJECT, ident, &[&w(0)]),
        }
    }

    fn opened(&mut self, i: usize, changes: &mut Changes) {
        let Some(c) = self.list[i].as_mut() else { return };
        if c.ours && c.theirs && c.state == State::Config {
            c.state = State::Open;
            changes.push(Change::Opened { handle: c.handle, local: c.local, psm: c.psm, initiator: c.initiator });
        }
    }
}

/// LE connection parameters a peripheral asks for (Vol 3 Part A 4.20): intervals in 1.25 ms, latency, timeout in 10 ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameters { pub min: u16, pub max: u16, pub latency: u16, pub timeout: u16 }

/// The LE signalling channel's PDU payload: a parameter update request is accepted when its values are valid (and
/// returned for the stack to apply), anything else refused as not understood.
pub fn le_signal(ctl: &mut Ctl, handle: u16, data: &[u8]) -> Option<Parameters> {
    if data.len() < 4 { return None; }
    let (code, ident, len) = (data[0], data[1], le16(data, 2) as usize);
    let b = data.get(4..4 + len)?;
    match code {
        PARAMETER_UPDATE_REQUEST if b.len() >= 8 => {
            let p = Parameters { min: le16(b, 0), max: le16(b, 2), latency: le16(b, 4), timeout: le16(b, 6) };
            let valid = (6..=3200).contains(&p.min) && p.min <= p.max && p.max <= 3200 && p.latency <= 499 && (10..=3200).contains(&p.timeout)
                && (p.timeout as u32) * 4 > (1 + p.latency as u32) * p.max as u32;
            command(ctl, handle, LE_SIGNALLING, PARAMETER_UPDATE_RESPONSE, ident, &[&w(if valid { 0 } else { 1 })]);
            valid.then_some(p)
        }
        COMMAND_REJECT | PARAMETER_UPDATE_RESPONSE => None,
        _ => { command(ctl, handle, LE_SIGNALLING, COMMAND_REJECT, ident, &[&w(0)]); None }
    }
}
