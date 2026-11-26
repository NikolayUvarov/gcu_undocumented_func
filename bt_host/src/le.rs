//! The LE central (700-BLT-0009): links to the LE devices listed, their security by SMP as initiator (Core 5.3 Vol 3
//! Part H: LE Secure Connections Just Works with P-256, else legacy Just Works; keys distributed kept, a later link
//! encrypted with them), then the ATT client (Part F) finds the HID service (HOGP 1.0) and turns on its input reports:
//! the boot protocol's keyboard and mouse reports where it has them, else the report protocol's with the report map.
//! A peer's own ATT requests are answered as by a server with no attributes.
use crate::buf::{le16, Addr, Hex};
use crate::ctl::Ctl;
use crate::devices::{Devices, Ltk};
use crate::hci;
use crate::l2cap;
use crate::smp_crypto::{self as sc, Key};
use crate::stack::{Ups, UP_KEYBOARD, UP_KEYS, UP_MAP, UP_POINTER, UP_REPORT};

pub const LINKS: usize = 2;
/// SMP commands.
pub const PAIRING_REQUEST: u8 = 0x01;
pub const PAIRING_RESPONSE: u8 = 0x02;
pub const PAIRING_CONFIRM: u8 = 0x03;
pub const PAIRING_RANDOM: u8 = 0x04;
pub const PAIRING_FAILED: u8 = 0x05;
pub const ENCRYPTION_INFORMATION: u8 = 0x06;
pub const CENTRAL_IDENTIFICATION: u8 = 0x07;
pub const IDENTITY_INFORMATION: u8 = 0x08;
pub const IDENTITY_ADDRESS: u8 = 0x09;
pub const SIGNING_INFORMATION: u8 = 0x0A;
pub const SECURITY_REQUEST: u8 = 0x0B;
pub const PUBLIC_KEY: u8 = 0x0C;
pub const DHKEY_CHECK: u8 = 0x0D;
/// Pairing Failed reasons.
pub const CONFIRM_FAILED: u8 = 0x04;
pub const PAIRING_NOT_SUPPORTED: u8 = 0x05;
pub const KEY_SIZE: u8 = 0x06;
pub const UNSPECIFIED: u8 = 0x08;
pub const INVALID_PARAMETERS: u8 = 0x0A;
pub const DHKEY_CHECK_FAILED: u8 = 0x0B;
/// Our pairing request: NoInputNoOutput, no OOB, bonding and Secure Connections, 16-octet keys, none distributed by us,
/// the peripheral's LTK and identity asked for.
pub const IO_CAPABILITY: u8 = 0x03;
pub const AUTH_REQ: u8 = 0x09;
pub const RESPONDER_KEYS: u8 = 0x03;
pub const SMP_MS: u64 = 30_000;

/// ATT opcodes.
pub const ERROR: u8 = 0x01;
pub const MTU_REQUEST: u8 = 0x02;
pub const MTU_RESPONSE: u8 = 0x03;
pub const FIND_INFORMATION: u8 = 0x04;
pub const FIND_INFORMATION_RESPONSE: u8 = 0x05;
pub const FIND_BY_TYPE: u8 = 0x06;
pub const FIND_BY_TYPE_RESPONSE: u8 = 0x07;
pub const READ_BY_TYPE: u8 = 0x08;
pub const READ_BY_TYPE_RESPONSE: u8 = 0x09;
pub const READ: u8 = 0x0A;
pub const READ_RESPONSE: u8 = 0x0B;
pub const READ_BLOB: u8 = 0x0C;
pub const READ_BLOB_RESPONSE: u8 = 0x0D;
pub const READ_BY_GROUP: u8 = 0x10;
pub const WRITE_REQUEST: u8 = 0x12;
pub const WRITE_RESPONSE: u8 = 0x13;
pub const NOTIFICATION: u8 = 0x1B;
pub const INDICATION: u8 = 0x1D;
pub const CONFIRMATION: u8 = 0x1E;
pub const WRITE_COMMAND: u8 = 0x52;
pub const ATTRIBUTE_NOT_FOUND: u8 = 0x0A;
pub const REQUEST_NOT_SUPPORTED: u8 = 0x06;
/// The ATT MTU we ask for: an LE data length of 251 less L2CAP's header.
pub const MTU: u16 = 247;
/// GATT and HOGP UUIDs.
pub const PRIMARY_SERVICE: u16 = 0x2800;
pub const CHARACTERISTIC: u16 = 0x2803;
pub const HID_SERVICE: u16 = 0x1812;
pub const REPORT_MAP: u16 = 0x2A4B;
pub const REPORT: u16 = 0x2A4D;
pub const PROTOCOL_MODE: u16 = 0x2A4E;
pub const BOOT_KEYBOARD_INPUT: u16 = 0x2A22;
pub const BOOT_MOUSE_INPUT: u16 = 0x2A33;
pub const CLIENT_CONFIGURATION: u16 = 0x2902;
pub const REPORT_REFERENCE: u16 = 0x2908;
pub const CHARACTERISTICS: usize = 24;
pub const MAP_MAX: usize = 512;
pub const ATT_MS: u64 = 30_000;

/// LE Create Connection's parameters for `addr`: scan every 60 ms for 30 ms, no filter list, a 30-50 ms connection
/// interval, no latency, a 5 s supervision timeout.
pub fn create_connection(addr: Addr, random: bool) -> [u8; 25] {
    let mut b = [0u8; 25];
    b[..4].copy_from_slice(&[0x60, 0x00, 0x30, 0x00]);
    b[5] = random as u8;
    b[6..12].copy_from_slice(&addr.0);
    for (i, v) in [0x0018u16, 0x0028, 0, 0x01F4, 0, 0].iter().enumerate() { b[13 + 2 * i..15 + 2 * i].copy_from_slice(&v.to_le_bytes()); }
    b
}

/// A deterministic random bit generator: AES-128 in counter mode under a key the driver seeds from the processor's
/// random source (testing stage: no reseeding).
pub struct Drbg { key: Key, counter: u128, pub seeded: bool }

impl Drbg {
    pub const fn new() -> Self { Self { key: [0; 16], counter: 0, seeded: false } }
    pub fn seed(&mut self, seed: &[u8; 32]) {
        self.key.copy_from_slice(&seed[..16]);
        self.counter = u128::from_le_bytes(seed[16..].try_into().unwrap());
        self.seeded = true;
    }
    pub fn next(&mut self) -> Key { self.counter = self.counter.wrapping_add(1); sc::e(&self.key, &self.counter.to_be_bytes()) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pairing { Idle, Response, PublicKey, Confirm, Random, Check, Encrypting, Keys, Done, Failed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gatt { Idle, Mtu, Service, Characteristics, Descriptors, References, Map, Configure, Ready, Absent }

#[derive(Clone, Copy, Debug, Default)]
pub struct Characteristic { pub value: u16, pub uuid: u16, pub properties: u8, pub cccd: u16, pub reference: u16, pub id: u8, pub kind: u8 }

pub struct Link {
    pub handle: u16, pub addr: Addr, pub random: bool, pub encrypted: bool,
    pub pairing: Pairing, since: u64, sc: bool, size: usize,
    preq: [u8; 7], pres: [u8; 7],
    secret: Option<p256::SecretKey>, ours: ([u8; 32], [u8; 32]), theirs: ([u8; 32], [u8; 32]), dhkey: [u8; 32],
    na: Key, nb: Key, confirm: Key, mackey: Key, ltk: Option<Ltk>, expected: u8, got: u8,
    pub gatt: Gatt, att_at: u64, pub mtu: u16, service: (u16, u16), pub chars: [Characteristic; CHARACTERISTICS], pub count: usize, at: u16,
    pub map: [u8; MAP_MAX], pub map_len: usize, pub boot: bool, keyboard_id: Option<u8>, configured: usize, pub reports: u32,
}

pub struct Central { pub links: [Option<Link>; LINKS], pub local: Addr, pub rng: Drbg }

impl Default for Central { fn default() -> Self { Self::new() } }

/// An address as SMP's functions take it: its type, then the address most significant octet first.
fn a56(random: bool, a: Addr) -> [u8; 7] { let mut v = [0u8; 7]; v[0] = random as u8; for i in 0..6 { v[1 + i] = a.0[5 - i]; } v }
fn spec6(a: Addr) -> [u8; 6] { sc::rev(a.0) }
fn smp(ctl: &mut Ctl, handle: u16, parts: &[&[u8]]) { ctl.l2cap(handle, l2cap::SMP, parts); }
fn att(ctl: &mut Ctl, handle: u16, parts: &[&[u8]]) { ctl.l2cap(handle, l2cap::ATT, parts); }

/// A short key kept to its size: its most significant octets zeroed (Vol 3 Part H 3.5.1).
fn sized(mut k: Key, size: usize) -> Key { for b in k.iter_mut().take(16 - size.min(16)) { *b = 0; } k }

/// The Report ID of the keyboard application collection in a report map, if it has one with an ID.
pub fn keyboard_id(map: &[u8]) -> Option<u8> {
    let (mut page, mut usage, mut inside, mut depth, mut at) = (0u32, 0u32, false, 0i32, 0);
    while at < map.len() {
        let prefix = map[at];
        if prefix == 0xFE { at += 3 + map.get(at + 1).copied().unwrap_or(0) as usize; continue; }
        let size = [0, 1, 2, 4][(prefix & 3) as usize];
        let data = map.get(at + 1..at + 1 + size)?;
        let value = data.iter().rev().fold(0u32, |v, &b| v << 8 | b as u32);
        match prefix & 0xFC {
            0x04 => page = value,
            0x08 => usage = value,
            0xA0 => { if depth == 0 && value == 1 { inside = page == 1 && usage == 6; } depth += 1; }
            0xC0 => { depth -= 1; if depth == 0 { inside = false; } }
            0x84 if inside => return Some(value as u8),
            _ => {}
        }
        at += 1 + size;
    }
    None
}

impl Central {
    pub const fn new() -> Self { Self { links: [const { None }; LINKS], local: Addr([0; 6]), rng: Drbg::new() } }

    fn link(&mut self, handle: u16) -> Option<&mut Link> { self.links.iter_mut().flatten().find(|l| l.handle == handle) }

    pub fn link_up(&mut self, ctl: &mut Ctl, handle: u16, addr: Addr, random: bool, ltk: Option<Ltk>) {
        let Some(slot) = self.links.iter_mut().find(|l| l.is_none()) else { return };
        *slot = Some(Link { handle, addr, random, encrypted: false, pairing: Pairing::Idle, since: ctl.now, sc: false, size: 16, preq: [0; 7], pres: [0; 7], secret: None,
            ours: ([0; 32], [0; 32]), theirs: ([0; 32], [0; 32]), dhkey: [0; 32], na: [0; 16], nb: [0; 16], confirm: [0; 16], mackey: [0; 16], ltk, expected: 0, got: 0,
            gatt: Gatt::Idle, att_at: 0, mtu: 23, service: (0, 0), chars: [Characteristic::default(); CHARACTERISTICS], count: 0, at: 0, map: [0; MAP_MAX], map_len: 0,
            boot: false, keyboard_id: None, configured: 0, reports: 0 });
        match ltk {
            Some(k) => { ctl.say(format_args!("LE {}: ENCRYPTING WITH ITS KEY", addr)); start_encryption(ctl, handle, &k); }
            None => self.pair(ctl, handle),
        }
    }

    /// Our Pairing Request.
    fn pair(&mut self, ctl: &mut Ctl, handle: u16) {
        let seeded = self.rng.seeded;
        let Some(l) = self.link(handle) else { return };
        if !seeded { ctl.say(format_args!("LE {}: NO RANDOM SOURCE: NOT PAIRED", l.addr)); l.pairing = Pairing::Failed; return; }
        let air = [PAIRING_REQUEST, IO_CAPABILITY, 0, AUTH_REQ, 16, 0, RESPONDER_KEYS];
        (l.preq, l.pairing, l.since) = (sc::rev(air), Pairing::Response, ctl.now);
        ctl.say(format_args!("LE {}: PAIRING (JUST WORKS, SECURE CONNECTIONS ASKED)", l.addr));
        smp(ctl, handle, &[&air]);
    }

    fn fail(ctl: &mut Ctl, l: &mut Link, reason: u8, why: &str) {
        ctl.say(format_args!("LE {}: PAIRING FAILED: {} (REASON {:02X})", l.addr, why, reason));
        smp(ctl, l.handle, &[&[PAIRING_FAILED, reason]]);
        l.pairing = Pairing::Failed;
    }

    pub fn link_down(&mut self, handle: u16) { for s in self.links.iter_mut() { if s.as_ref().is_some_and(|l| l.handle == handle) { *s = None; } } }

    /// Encryption Change on an LE link: with a key distributed still to come it waits for them, else GATT starts. A
    /// key the device refused (PIN or key missing) is forgotten and the link paired again.
    pub fn encrypted(&mut self, ctl: &mut Ctl, ups: &mut Ups, devices: &mut Devices, handle: u16, status: u8, on: bool) {
        let Some(l) = self.link(handle) else { return };
        l.encrypted = status == 0 && on;
        if !l.encrypted {
            ctl.say(format_args!("LE {}: ENCRYPTION FAILED (STATUS {:02X})", l.addr, status));
            if status == 0x06 && l.pairing == Pairing::Idle {
                if let Some(d) = devices.get_mut(l.addr) { d.ltk = None; ups.push(UP_KEYS, &[]); }
                ctl.say(format_args!("LE {}: ITS KEY REFUSED: FORGOTTEN, PAIRING AGAIN", l.addr));
                self.pair(ctl, handle);
            }
            return;
        }
        ctl.say(format_args!("LE {}: ENCRYPTED", l.addr));
        if l.pairing == Pairing::Encrypting {
            if let (true, Some(k)) = (l.sc, l.ltk) { if let Some(d) = devices.get_mut(l.addr) { d.ltk = Some(k); ups.push(UP_KEYS, &[]); } }
            l.pairing = if l.expected & !l.got & 0x03 != 0 { Pairing::Keys } else { Pairing::Done };
            if l.pairing == Pairing::Done { ctl.say(format_args!("LE {}: PAIRING DONE", l.addr)); }
        }
        if l.pairing != Pairing::Keys { start_gatt(ctl, l); }
    }

    /// An SMP or ATT PDU from `handle`.
    pub fn pdu(&mut self, ctl: &mut Ctl, ups: &mut Ups, devices: &mut Devices, handle: u16, cid: u16, p: &[u8]) {
        if cid == l2cap::SMP { self.smp(ctl, ups, devices, handle, p) } else { self.att(ctl, ups, handle, p) }
    }

    fn smp(&mut self, ctl: &mut Ctl, ups: &mut Ups, devices: &mut Devices, handle: u16, p: &[u8]) {
        let local = self.local;
        let r16 = |rng: &mut Drbg| rng.next();
        let Some(&code) = p.first() else { return };
        // The random values a step needs, drawn before the link is borrowed.
        let (n, k) = (r16(&mut self.rng), [r16(&mut self.rng), r16(&mut self.rng)]);
        let Some(l) = self.link(handle) else { return };
        let a_init = a56(false, local);
        let a_resp = a56(l.random, l.addr);
        match (code, l.pairing) {
            (SECURITY_REQUEST, Pairing::Idle | Pairing::Failed | Pairing::Done) => {
                let ltk = devices.get(l.addr).and_then(|d| d.ltk);
                match ltk { Some(k) if !l.encrypted => start_encryption(ctl, handle, &k), Some(_) => {}, None => self.pair(ctl, handle) }
            }
            (PAIRING_RESPONSE, Pairing::Response) if p.len() >= 7 => {
                let air: [u8; 7] = p[..7].try_into().unwrap();
                l.pres = sc::rev(air);
                l.size = p[4] as usize;
                l.expected = p[6] & RESPONDER_KEYS;
                if !(7..=16).contains(&l.size) { Self::fail(ctl, l, KEY_SIZE, "KEY SIZE"); return; }
                l.sc = AUTH_REQ & p[3] & 0x08 != 0;
                ctl.say(format_args!("LE {}: ITS IO CAPABILITY {}, AUTHENTICATION {:02X}, {}", l.addr, p[1], p[3], if l.sc { "SECURE CONNECTIONS" } else { "LEGACY PAIRING" }));
                if l.sc {
                    // A P-256 key pair from two draws; a draw that is no valid secret is drawn again by the next pairing.
                    let mut seed = [0u8; 32];
                    seed[..16].copy_from_slice(&k[0]);
                    seed[16..].copy_from_slice(&k[1]);
                    let Some((secret, x, y)) = sc::keypair(&seed) else { Self::fail(ctl, l, UNSPECIFIED, "NO KEY PAIR"); return };
                    (l.secret, l.ours, l.pairing) = (Some(secret), (x, y), Pairing::PublicKey);
                    smp(ctl, handle, &[&[PUBLIC_KEY], &sc::rev(x), &sc::rev(y)]);
                } else {
                    // Just Works: TK zero.
                    l.na = n;
                    let c = sc::c1(&[0; 16], &l.na, &l.preq, &l.pres, 0, &spec6(local), l.random as u8, &spec6(l.addr));
                    l.pairing = Pairing::Confirm;
                    smp(ctl, handle, &[&[PAIRING_CONFIRM], &sc::rev(c)]);
                }
            }
            (PUBLIC_KEY, Pairing::PublicKey) if p.len() >= 65 => {
                let x: [u8; 32] = sc::rev(p[1..33].try_into().unwrap());
                let y: [u8; 32] = sc::rev(p[33..65].try_into().unwrap());
                let Some(dh) = l.secret.as_ref().and_then(|s| sc::dhkey(s, &x, &y)) else { Self::fail(ctl, l, INVALID_PARAMETERS, "ITS PUBLIC KEY IS NOT ON THE CURVE"); return };
                (l.theirs, l.dhkey, l.secret, l.pairing) = ((x, y), dh, None, Pairing::Confirm);
            }
            (PAIRING_CONFIRM, Pairing::Confirm) if p.len() >= 17 => {
                l.confirm = sc::rev(p[1..17].try_into().unwrap());
                if l.sc { l.na = n; }
                l.pairing = Pairing::Random;
                smp(ctl, handle, &[&[PAIRING_RANDOM], &sc::rev(l.na)]);
            }
            (PAIRING_RANDOM, Pairing::Random) if p.len() >= 17 => {
                l.nb = sc::rev(p[1..17].try_into().unwrap());
                if l.sc {
                    if sc::f4(&l.theirs.0, &l.ours.0, &l.nb, 0) != l.confirm { Self::fail(ctl, l, CONFIRM_FAILED, "ITS CONFIRM VALUE"); return; }
                    let number = sc::g2(&l.ours.0, &l.theirs.0, &l.na, &l.nb) % 1_000_000;
                    ctl.say(format_args!("LE {}: JUST WORKS ({:06} NOT COMPARED: THE TESTING STAGE)", l.addr, number));
                    let (mackey, ltk) = sc::f5(&l.dhkey, &l.na, &l.nb, &a_init, &a_resp);
                    l.mackey = mackey;
                    l.ltk = Some(Ltk { key: sc::rev(sized(ltk, l.size)), ediv: 0, rand: [0; 8], authenticated: false });
                    let io = [AUTH_REQ, 0, IO_CAPABILITY];
                    let ea = sc::f6(&l.mackey, &l.na, &l.nb, &[0; 16], &io, &a_init, &a_resp);
                    l.pairing = Pairing::Check;
                    smp(ctl, handle, &[&[DHKEY_CHECK], &sc::rev(ea)]);
                } else {
                    let c = sc::c1(&[0; 16], &l.nb, &l.preq, &l.pres, 0, &spec6(local), l.random as u8, &spec6(l.addr));
                    if c != l.confirm { Self::fail(ctl, l, CONFIRM_FAILED, "ITS CONFIRM VALUE"); return; }
                    let stk = sized(sc::s1(&[0; 16], &l.nb, &l.na), l.size);
                    l.pairing = Pairing::Encrypting;
                    start_encryption(ctl, handle, &Ltk { key: sc::rev(stk), ediv: 0, rand: [0; 8], authenticated: false });
                }
            }
            (DHKEY_CHECK, Pairing::Check) if p.len() >= 17 => {
                let eb: Key = sc::rev(p[1..17].try_into().unwrap());
                let io_b = [l.pres[3], l.pres[4], l.pres[5]];
                if sc::f6(&l.mackey, &l.nb, &l.na, &[0; 16], &io_b, &a_resp, &a_init) != eb { Self::fail(ctl, l, DHKEY_CHECK_FAILED, "ITS DHKEY CHECK"); return; }
                l.pairing = Pairing::Encrypting;
                // Secure Connections distributes no LTK: ours is the one made.
                l.expected &= !0x01;
                let k = l.ltk.unwrap();
                start_encryption(ctl, handle, &k);
            }
            (ENCRYPTION_INFORMATION, Pairing::Keys | Pairing::Encrypting) if p.len() >= 17 => {
                let mut k = Ltk::default();
                k.key.copy_from_slice(&p[1..17]);
                l.ltk = Some(k);
            }
            (CENTRAL_IDENTIFICATION, Pairing::Keys | Pairing::Encrypting) if p.len() >= 11 => {
                let mut k = l.ltk.unwrap_or_default();
                k.ediv = le16(p, 1);
                k.rand.copy_from_slice(&p[3..11]);
                l.ltk = Some(k);
                l.got |= 0x01;
                if let Some(d) = devices.get_mut(l.addr) { d.ltk = Some(k); ups.push(UP_KEYS, &[]); }
                ctl.say(format_args!("LE {}: ITS LONG TERM KEY KEPT", l.addr));
            }
            (IDENTITY_INFORMATION, Pairing::Keys | Pairing::Encrypting) => {}
            (IDENTITY_ADDRESS, Pairing::Keys | Pairing::Encrypting) if p.len() >= 8 => {
                l.got |= 0x02;
                ctl.say(format_args!("LE {}: ITS IDENTITY ADDRESS {} ({})", l.addr, Addr::from_wire(&p[2..8]), if p[1] == 0 { "PUBLIC" } else { "RANDOM STATIC" }));
            }
            (SIGNING_INFORMATION, _) => {}
            (PAIRING_FAILED, _) => {
                ctl.say(format_args!("LE {}: THE DEVICE FAILED THE PAIRING (REASON {:02X})", l.addr, p.get(1).copied().unwrap_or(0)));
                l.pairing = Pairing::Failed;
            }
            (PAIRING_REQUEST, _) => smp(ctl, handle, &[&[PAIRING_FAILED, PAIRING_NOT_SUPPORTED]]),
            _ => {}
        }
        // Every key it promised came: done, GATT starts.
        if let Some(l) = self.link(handle) {
            if l.pairing == Pairing::Keys && l.expected & !l.got & 0x03 == 0 {
                l.pairing = Pairing::Done;
                ctl.say(format_args!("LE {}: PAIRING DONE", l.addr));
                start_gatt(ctl, l);
            }
        }
    }

    fn att(&mut self, ctl: &mut Ctl, ups: &mut Ups, handle: u16, p: &[u8]) {
        let Some(&op) = p.first() else { return };
        let Some(l) = self.link(handle) else { return };
        match op {
            // The peer's requests: a server with no attributes.
            MTU_REQUEST => att(ctl, handle, &[&[MTU_RESPONSE], &MTU.to_le_bytes()]),
            FIND_INFORMATION | FIND_BY_TYPE | READ_BY_TYPE | READ_BY_GROUP => att(ctl, handle, &[&[ERROR, op, p.get(1).copied().unwrap_or(0), p.get(2).copied().unwrap_or(0), ATTRIBUTE_NOT_FOUND]]),
            NOTIFICATION if p.len() >= 3 => report(ctl, ups, l, le16(p, 1), &p[3..]),
            INDICATION if p.len() >= 3 => { att(ctl, handle, &[&[CONFIRMATION]]); report(ctl, ups, l, le16(p, 1), &p[3..]); }
            WRITE_COMMAND | CONFIRMATION => {}
            _ if op & 1 == 0 && op & 0x40 == 0 => att(ctl, handle, &[&[ERROR, op, p.get(1).copied().unwrap_or(0), p.get(2).copied().unwrap_or(0), REQUEST_NOT_SUPPORTED]]),
            _ => gatt_answer(ctl, l, op, p),
        }
    }

    /// GATT that took too long, and pairing that did: the link is let go.
    pub fn tick(&mut self, ctl: &mut Ctl) {
        for l in self.links.iter_mut().flatten() {
            let pairing = !matches!(l.pairing, Pairing::Idle | Pairing::Done | Pairing::Failed);
            if pairing && ctl.now > l.since + SMP_MS { ctl.say(format_args!("LE {}: PAIRING TIMED OUT IN {:?}", l.addr, l.pairing)); l.pairing = Pairing::Failed; ctl.command(hci::DISCONNECT, &[l.handle as u8, (l.handle >> 8) as u8, 0x13]); }
            let waiting = !matches!(l.gatt, Gatt::Idle | Gatt::Ready | Gatt::Absent);
            if waiting && ctl.now > l.att_at + ATT_MS { ctl.say(format_args!("LE {}: NO ATT ANSWER IN {:?}", l.addr, l.gatt)); l.gatt = Gatt::Absent; }
        }
    }
}

fn start_encryption(ctl: &mut Ctl, handle: u16, k: &Ltk) {
    let mut b = [0u8; 28];
    b[..2].copy_from_slice(&handle.to_le_bytes());
    b[2..10].copy_from_slice(&k.rand);
    b[10..12].copy_from_slice(&k.ediv.to_le_bytes());
    b[12..].copy_from_slice(&k.key);
    ctl.command(hci::LE_START_ENCRYPTION, &b);
}

fn request(ctl: &mut Ctl, l: &mut Link, state: Gatt, parts: &[&[u8]]) { (l.gatt, l.att_at) = (state, ctl.now); att(ctl, l.handle, parts); }

fn start_gatt(ctl: &mut Ctl, l: &mut Link) {
    if l.gatt != Gatt::Idle { return; }
    request(ctl, l, Gatt::Mtu, &[&[MTU_REQUEST], &MTU.to_le_bytes()]);
}

fn w(v: u16) -> [u8; 2] { v.to_le_bytes() }

/// The next characteristic, from `l.at`, whose report reference is to be read, then the map, then the configuration.
fn next_reference(ctl: &mut Ctl, l: &mut Link) {
    while (l.at as usize) < l.count {
        let c = l.chars[l.at as usize];
        if c.uuid == REPORT && c.reference != 0 { request(ctl, l, Gatt::References, &[&[READ], &w(c.reference)]); return; }
        l.at += 1;
    }
    match l.chars[..l.count].iter().find(|c| c.uuid == REPORT_MAP).map(|c| c.value) {
        Some(map) => request(ctl, l, Gatt::Map, &[&[READ], &w(map)]),
        None => configure(ctl, l),
    }
}

/// The input reports' notifications turned on, one write at a time; the boot protocol's where the device has them.
fn configure(ctl: &mut Ctl, l: &mut Link) {
    if l.configured == 0 {
        l.boot = l.chars[..l.count].iter().any(|c| matches!(c.uuid, BOOT_KEYBOARD_INPUT | BOOT_MOUSE_INPUT) && c.cccd != 0);
        if l.map_len > 0 { l.keyboard_id = keyboard_id(&l.map[..l.map_len]); }
        if let Some(pm) = l.chars[..l.count].iter().find(|c| c.uuid == PROTOCOL_MODE) {
            // Boot (0) or report (1) protocol, without an answer.
            att(ctl, l.handle, &[&[WRITE_COMMAND], &w(pm.value), &[!l.boot as u8]]);
        }
        ctl.say(format_args!("LE {}: HID SERVICE {:04X}-{:04X}, {} CHARACTERISTICS, {} PROTOCOL, REPORT MAP {} BYTES", l.addr, l.service.0, l.service.1, l.count,
            if l.boot { "BOOT" } else { "REPORT" }, l.map_len));
    }
    let boot = l.boot;
    let wanted = |c: &Characteristic| c.cccd != 0 && if boot { matches!(c.uuid, BOOT_KEYBOARD_INPUT | BOOT_MOUSE_INPUT) } else { c.uuid == REPORT && c.kind == 1 };
    while l.configured < l.count {
        let c = l.chars[l.configured];
        l.configured += 1;
        if wanted(&c) { request(ctl, l, Gatt::Configure, &[&[WRITE_REQUEST], &w(c.cccd), &[1, 0]]); return; }
    }
    l.gatt = Gatt::Ready;
    ctl.say(format_args!("LE {}: HID READY", l.addr));
}

/// A response to our GATT request.
fn gatt_answer(ctl: &mut Ctl, l: &mut Link, op: u8, p: &[u8]) {
    let not_found = op == ERROR && p.get(4) == Some(&ATTRIBUTE_NOT_FOUND);
    match (l.gatt, op) {
        (Gatt::Mtu, MTU_RESPONSE | ERROR) => {
            if op == MTU_RESPONSE { l.mtu = le16(p, 1).clamp(23, MTU); }
            request(ctl, l, Gatt::Service, &[&[FIND_BY_TYPE], &w(1), &w(0xFFFF), &w(PRIMARY_SERVICE), &w(HID_SERVICE)]);
        }
        (Gatt::Service, FIND_BY_TYPE_RESPONSE) if p.len() >= 5 => {
            l.service = (le16(p, 1), le16(p, 3));
            l.at = l.service.0;
            request(ctl, l, Gatt::Characteristics, &[&[READ_BY_TYPE], &w(l.service.0), &w(l.service.1), &w(CHARACTERISTIC)]);
        }
        (Gatt::Service, ERROR) => { l.gatt = Gatt::Absent; ctl.say(format_args!("LE {}: NO HID SERVICE", l.addr)); }
        (Gatt::Characteristics, READ_BY_TYPE_RESPONSE) if p.len() >= 2 => {
            let size = p[1] as usize;
            let mut last = l.at;
            // Declaration handle, properties, value handle, a 16-bit UUID (a 128-bit one is kept without its UUID).
            for e in p[2..].chunks_exact(size.max(1)).filter(|_| size >= 7) {
                last = le16(e, 0);
                if l.count < CHARACTERISTICS {
                    l.chars[l.count] = Characteristic { value: le16(e, 3), uuid: if size == 7 { le16(e, 5) } else { 0 }, properties: e[2], ..Default::default() };
                    l.count += 1;
                }
            }
            if last >= l.service.1 || last < l.at { descriptors(ctl, l, 0); return; }
            l.at = last + 1;
            request(ctl, l, Gatt::Characteristics, &[&[READ_BY_TYPE], &w(l.at), &w(l.service.1), &w(CHARACTERISTIC)]);
        }
        (Gatt::Characteristics, ERROR) if not_found || l.count > 0 => descriptors(ctl, l, 0),
        (Gatt::Descriptors, FIND_INFORMATION_RESPONSE) if p.len() >= 2 => {
            let size = if p[1] == 1 { 4 } else { 18 };
            let mut last = l.at;
            for e in p[2..].chunks_exact(size) {
                let handle = le16(e, 0);
                last = handle;
                let uuid = if size == 4 { le16(e, 2) } else { 0 };
                // A descriptor belongs to the characteristic before it.
                if let Some(c) = l.chars[..l.count].iter_mut().rev().find(|c| c.value < handle) {
                    match uuid { CLIENT_CONFIGURATION => c.cccd = handle, REPORT_REFERENCE => c.reference = handle, _ => {} }
                }
            }
            if last >= l.service.1 || last < l.at { (l.at, l.gatt) = (0, Gatt::References); next_reference(ctl, l); return; }
            descriptors(ctl, l, last + 1);
        }
        (Gatt::Descriptors, ERROR) => { l.at = 0; next_reference(ctl, l); }
        (Gatt::References, READ_RESPONSE) if p.len() >= 3 => {
            let at = l.at as usize;
            (l.chars[at].id, l.chars[at].kind) = (p[1], p[2]);
            l.at += 1;
            next_reference(ctl, l);
        }
        (Gatt::References, ERROR) => { l.at += 1; next_reference(ctl, l); }
        (Gatt::Map, READ_RESPONSE | READ_BLOB_RESPONSE) => {
            let part = &p[1..];
            let n = part.len().min(MAP_MAX - l.map_len);
            l.map[l.map_len..l.map_len + n].copy_from_slice(&part[..n]);
            l.map_len += n;
            // A full answer may have more: Read Blob from where it ended.
            let map = l.chars[..l.count].iter().find(|c| c.uuid == REPORT_MAP).map_or(0, |c| c.value);
            if part.len() == l.mtu as usize - 1 && l.map_len < MAP_MAX { request(ctl, l, Gatt::Map, &[&[READ_BLOB], &w(map), &w(l.map_len as u16)]); } else { configure(ctl, l); }
        }
        (Gatt::Map, ERROR) => configure(ctl, l),
        (Gatt::Configure, WRITE_RESPONSE | ERROR) => configure(ctl, l),
        _ => {}
    }
}

fn descriptors(ctl: &mut Ctl, l: &mut Link, from: u16) {
    let first = if from == 0 { l.chars[..l.count].first().map_or(l.service.0, |c| c.value + 1) } else { from };
    l.at = first;
    if first > l.service.1 { l.at = 0; l.gatt = Gatt::References; next_reference(ctl, l); return; }
    request(ctl, l, Gatt::Descriptors, &[&[FIND_INFORMATION], &w(first), &w(l.service.1)]);
}

/// A notified value: a boot report as it is, a report protocol's keyboard report as a boot one, any other report with
/// its ID before it for the driver's layout of the report map.
fn report(ctl: &mut Ctl, ups: &mut Ups, l: &mut Link, handle: u16, value: &[u8]) {
    let Some(c) = l.chars[..l.count].iter().find(|c| c.value == handle).copied() else { return };
    l.reports += 1;
    if l.reports == 1 {
        ctl.say(format_args!("LE {}: FIRST REPORT ON {:04X} ({:04X}): {}", l.addr, handle, c.uuid, Hex(&value[..value.len().min(12)])));
        if !l.boot && l.map_len > 0 { ups.push(UP_MAP, &l.map[..l.map_len]); }
    }
    match c.uuid {
        BOOT_KEYBOARD_INPUT if value.len() >= 8 => ups.push(UP_KEYBOARD, &value[..8]),
        BOOT_MOUSE_INPUT if value.len() >= 3 => ups.push(UP_POINTER, &value[..value.len().min(8)]),
        REPORT if Some(c.id) == l.keyboard_id && value.len() >= 8 => ups.push(UP_KEYBOARD, &value[..8]),
        REPORT => { let n = value.len().min(62); ups.queue.push(&[&[UP_REPORT, c.id], &value[..n]]); }
        _ => {}
    }
}
