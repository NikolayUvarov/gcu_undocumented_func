//! A controller for the tests: it takes the stack's commands and ACL packets, answers commands as a controller
//! would (status 0, the buffer sizes and features given), returns ACL credits, and builds the events and ACL packets
//! a peer's controller would send.
#![allow(dead_code)]
use bt_host::buf::Addr;
use bt_host::hci;
use bt_host::stack::{Stack, Up};

/// Commands answered by Command Status rather than Command Complete.
const BY_STATUS: [u16; 11] = [hci::CREATE_CONNECTION, hci::DISCONNECT, hci::ACCEPT_CONNECTION, hci::REJECT_CONNECTION, hci::AUTHENTICATION_REQUESTED,
    hci::SET_CONNECTION_ENCRYPTION, hci::INQUIRY, hci::LE_CREATE_CONNECTION, hci::LE_CONNECTION_UPDATE, hci::LE_START_ENCRYPTION, hci::REMOTE_NAME_REQUEST];

pub const PEER: Addr = Addr([0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
pub const ACL_LEN: u16 = 310;

pub struct Sim { pub stack: Stack, pub now: u64, pub acl_len: u16, pub commands: Vec<(u16, Vec<u8>)>, pub log: Vec<String> }

impl Sim {
    /// A stack with `devices` listed, started, its setup answered.
    pub fn new(devices: &str) -> Self {
        let mut s = Sim { stack: Stack::new(), now: 1_000, acl_len: ACL_LEN, commands: Vec::new(), log: Vec::new() };
        s.stack.devices.load(devices, |n, l| panic!("line {} not understood: {}", n, l));
        s.stack.start(s.now);
        s.run();
        s
    }

    /// Takes every command the stack has, answering each; returns them.
    pub fn run(&mut self) -> Vec<(u16, Vec<u8>)> {
        let mut got = Vec::new();
        let mut out = [0u8; 300];
        while let Some(n) = self.stack.ctl.next_command(&mut out) {
            let (op, params) = (u16::from_le_bytes([out[0], out[1]]), out[3..n].to_vec());
            assert_eq!(out[2] as usize, n - 3, "the length byte of {:04X}", op);
            got.push((op, params));
            if BY_STATUS.contains(&op) { self.stack.event(hci::EV_COMMAND_STATUS, &[0, 1, op as u8, (op >> 8) as u8]); continue; }
            let ret: Vec<u8> = match op {
                hci::READ_BUFFER_SIZE => vec![0, self.acl_len as u8, (self.acl_len >> 8) as u8, 64, 8, 0, 0, 0],
                // LE (bit 38) and Secure Simple Pairing (bit 51).
                hci::READ_LOCAL_FEATURES => vec![0, 0xFF, 0xFF, 0xFF, 0xFE, 0xDB, 0xFF, 0x7B, 0x87],
                hci::LE_READ_BUFFER_SIZE => vec![0, 251, 0, 4],
                _ => vec![0],
            };
            let mut ev = vec![1, op as u8, (op >> 8) as u8];
            ev.extend_from_slice(&ret);
            self.stack.event(hci::EV_COMMAND_COMPLETE, &ev);
        }
        self.drain_log();
        self.commands.extend(got.iter().cloned());
        got
    }

    pub fn drain_log(&mut self) {
        let mut line = [0u8; bt_host::buf::LINE];
        while let Some(l) = self.stack.ctl.log.take(&mut line) { self.log.push(l.to_string()); }
    }

    pub fn logged(&mut self, needle: &str) -> bool { self.drain_log(); self.log.iter().any(|l| l.contains(needle)) }

    pub fn tick(&mut self, ms: u64) -> Vec<(u16, Vec<u8>)> { self.now += ms; self.stack.tick(self.now); self.run() }

    pub fn event(&mut self, code: u8, params: &[u8]) -> Vec<(u16, Vec<u8>)> { self.stack.event(code, params); self.run() }

    /// The ACL packets the stack sends, each acknowledged at once with Number Of Completed Packets.
    pub fn acl(&mut self) -> Vec<(u16, u8, Vec<u8>)> {
        let mut got = Vec::new();
        let mut out = [0u8; 2100];
        while let Some(n) = self.stack.ctl.next_acl(&mut out) {
            let (handle, pb, data) = bt_host::acl::parse(&out[..n]).expect("an ACL packet");
            assert!(data.len() <= self.acl_len as usize, "a fragment longer than the controller takes");
            got.push((handle, pb, data.to_vec()));
            self.stack.event(hci::EV_NUMBER_OF_COMPLETED_PACKETS, &[1, handle as u8, (handle >> 8) as u8, 1, 0]);
        }
        self.drain_log();
        got
    }

    /// The L2CAP frames the stack sends, reassembled: link, channel, payload.
    pub fn frames(&mut self) -> Vec<(u16, u16, Vec<u8>)> {
        let mut frames = Vec::new();
        let mut open: Option<(u16, Vec<u8>)> = None;
        for (handle, pb, data) in self.acl() {
            if pb != bt_host::acl::PB_CONTINUING { assert!(open.is_none(), "a frame cut short"); open = Some((handle, data)); }
            else { open.as_mut().expect("a continuation without a start").1.extend_from_slice(&data); }
            let done = open.as_ref().is_some_and(|(_, b)| b.len() >= 4 && b.len() == 4 + u16::from_le_bytes([b[0], b[1]]) as usize);
            if done { let (h, b) = open.take().unwrap(); frames.push((h, u16::from_le_bytes([b[2], b[3]]), b[4..].to_vec())); }
        }
        assert!(open.is_none(), "a frame left unfinished");
        frames
    }

    /// An L2CAP frame from the peer, cut into ACL packets of `chunk` bytes.
    pub fn send(&mut self, handle: u16, cid: u16, payload: &[u8], chunk: usize) {
        let mut pdu = vec![payload.len() as u8, (payload.len() >> 8) as u8, cid as u8, (cid >> 8) as u8];
        pdu.extend_from_slice(payload);
        for (i, c) in pdu.chunks(chunk).enumerate() {
            let mut p = bt_host::acl::header(handle, if i == 0 { bt_host::acl::PB_FIRST } else { bt_host::acl::PB_CONTINUING }, c.len()).to_vec();
            p.extend_from_slice(c);
            self.stack.acl(&p);
        }
    }

    pub fn ups(&mut self) -> Vec<String> {
        let mut out = [0u8; bt_host::stack::UP_MAX];
        let mut got = Vec::new();
        while let Some(u) = self.stack.ups.take(&mut out) {
            got.push(match u {
                Up::Keys => "keys".to_string(), Up::Keyboard(r) => format!("keyboard {:02X?}", r), Up::Pointer(r) => format!("pointer {:02X?}", r),
                Up::Map(m) => format!("map of {} bytes", m.len()), Up::Report(id, r) => format!("report {} {:02X?}", id, r),
            });
        }
        got
    }

    /// Connection Complete for the peer on `handle`.
    pub fn connected(&mut self, handle: u16) -> Vec<(u16, Vec<u8>)> {
        let mut p = vec![0, handle as u8, (handle >> 8) as u8];
        p.extend_from_slice(&PEER.0);
        p.extend_from_slice(&[1, 0]);
        self.event(hci::EV_CONNECTION_COMPLETE, &p)
    }

    /// The pairing a controller runs after Authentication Requested with no key: Link Key Request, IO Capability
    /// Request, the peer's IO capability, User Confirmation Request, completion, the key, Authentication Complete.
    pub fn pair(&mut self, handle: u16) -> Vec<(u16, Vec<u8>)> {
        let a = PEER.0;
        let mut got = self.event(hci::EV_LINK_KEY_REQUEST, &a);
        got.extend(self.event(hci::EV_IO_CAPABILITY_REQUEST, &a));
        got.extend(self.event(hci::EV_IO_CAPABILITY_RESPONSE, &[a[0], a[1], a[2], a[3], a[4], a[5], 3, 0, 4]));
        got.extend(self.event(hci::EV_USER_CONFIRMATION_REQUEST, &[a[0], a[1], a[2], a[3], a[4], a[5], 0x40, 0xE2, 0x01, 0]));
        got.extend(self.event(hci::EV_SIMPLE_PAIRING_COMPLETE, &[0, a[0], a[1], a[2], a[3], a[4], a[5]]));
        let mut key = a.to_vec();
        key.extend((0..16u8).map(|i| 0xA0 + i));
        key.push(4);
        got.extend(self.event(hci::EV_LINK_KEY_NOTIFICATION, &key));
        got.extend(self.event(hci::EV_AUTHENTICATION_COMPLETE, &[0, handle as u8, (handle >> 8) as u8]));
        got
    }

    pub fn encrypted(&mut self, handle: u16) -> Vec<(u16, Vec<u8>)> { self.event(hci::EV_ENCRYPTION_CHANGE, &[0, handle as u8, (handle >> 8) as u8, 1]) }
}

pub fn ops(cmds: &[(u16, Vec<u8>)]) -> Vec<u16> { cmds.iter().map(|c| c.0).collect() }

/// An L2CAP signalling command.
pub fn sig(code: u8, ident: u8, data: &[u8]) -> Vec<u8> {
    let mut v = vec![code, ident, data.len() as u8, (data.len() >> 8) as u8];
    v.extend_from_slice(data);
    v
}

pub fn le16(b: &[u8], at: usize) -> u16 { u16::from_le_bytes([b[at], b[at + 1]]) }
