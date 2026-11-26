//! SDP (Core 5.3 Vol 3 Part B): data elements read and written, and a server that answers a peer's searches for the
//! records we have: the A2DP source's (A2DP 5.3), and AVRCP's controller for a sink's volume. A sink may look for it before it connects its own channels.
use crate::buf::be16;
use crate::ctl::Ctl;
use crate::l2cap::Channels;

pub const ERROR: u8 = 0x01;
pub const SEARCH_REQUEST: u8 = 0x02;
pub const SEARCH_RESPONSE: u8 = 0x03;
pub const ATTRIBUTE_REQUEST: u8 = 0x04;
pub const ATTRIBUTE_RESPONSE: u8 = 0x05;
pub const SEARCH_ATTRIBUTE_REQUEST: u8 = 0x06;
pub const SEARCH_ATTRIBUTE_RESPONSE: u8 = 0x07;

pub const INVALID_HANDLE: u16 = 0x0002;
pub const INVALID_SYNTAX: u16 = 0x0003;
pub const INVALID_CONTINUATION: u16 = 0x0005;

/// A data element's type (5 bits) and value: `head` is its header's length, `value` what follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Element<'a> { pub kind: u8, pub value: &'a [u8], pub len: usize }

pub const NIL: u8 = 0;
pub const UINT: u8 = 1;
pub const UUID: u8 = 3;
pub const TEXT: u8 = 4;
pub const SEQUENCE: u8 = 6;

/// The data element at the start of `b`.
pub fn element(b: &[u8]) -> Option<Element<'_>> {
    let first = *b.first()?;
    let (kind, size) = (first >> 3, first & 7);
    let (head, len) = match size {
        0..=4 if kind == NIL => (1, 0),
        0..=4 => (1, 1usize << size),
        5 => (2, *b.get(1)? as usize),
        6 => (3, be16(b, 1) as usize),
        _ => (5, crate::buf::be32(b, 1) as usize),
    };
    if b.len() < head + len { return None; }
    Some(Element { kind, value: &b[head..head + len], len: head + len })
}

/// The elements of a sequence, each passed to `each`; false when one is malformed.
pub fn each(mut seq: &[u8], mut f: impl FnMut(Element) -> bool) -> bool {
    while !seq.is_empty() {
        let Some(e) = element(seq) else { return false };
        if !f(e) { return false; }
        seq = &seq[e.len..];
    }
    true
}

/// A UUID element as 32 bits when it is 16 or 32 bits, or a 128-bit one on the Bluetooth base UUID.
pub fn uuid32(e: &Element) -> Option<u32> {
    const BASE: [u8; 12] = [0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB];
    if e.kind != UUID { return None; }
    match e.value.len() {
        2 => Some(be16(e.value, 0) as u32),
        4 => Some(crate::buf::be32(e.value, 0)),
        16 if e.value[4..] == BASE => Some(crate::buf::be32(e.value, 0)),
        _ => None,
    }
}

/// A record: its handle, the UUIDs it holds (for searches), its attributes encoded in ascending order (each an ID
/// element and its value).
pub struct Record { pub handle: u32, pub uuids: &'static [u32], pub attributes: &'static [(u16, &'static [u8])] }

/// The A2DP source's record: its class, L2CAP PSM 0x0019 and AVDTP 1.3, the public browse group, A2DP 1.3, a player.
pub const A2DP_SOURCE: Record = Record {
    handle: 0x0001_0001,
    uuids: &[0x110A, 0x0100, 0x0019, 0x1002, 0x110D],
    attributes: &[
        (0x0000, &[0x0A, 0x00, 0x01, 0x00, 0x01]),
        (0x0001, &[0x35, 0x03, 0x19, 0x11, 0x0A]),
        (0x0004, &[0x35, 0x10, 0x35, 0x06, 0x19, 0x01, 0x00, 0x09, 0x00, 0x19, 0x35, 0x06, 0x19, 0x00, 0x19, 0x09, 0x01, 0x03]),
        (0x0005, &[0x35, 0x03, 0x19, 0x10, 0x02]),
        (0x0009, &[0x35, 0x08, 0x35, 0x06, 0x19, 0x11, 0x0D, 0x09, 0x01, 0x03]),
        (0x0311, &[0x09, 0x00, 0x01]),
    ],
};

/// AVRCP's controller, as the source that sets a sink's volume (AVRCP 1.4's absolute volume is a category 2 feature):
/// A/V Remote Control and its Controller class, L2CAP PSM 0x0017 and AVCTP 1.4, AVRCP 1.4, category 2 (AVRCP 1.6.2
/// table 8.1, but for the profile version: 1.4, whose absolute volume is what the stack does). No target record: a
/// category 1 target owes the browsing channel and metadata the stack does not have (table 8.2, C1 and *3); a sink's
/// target commands are answered all the same.
pub const AVRCP_CONTROLLER: Record = Record {
    handle: 0x0001_0002,
    uuids: &[0x110E, 0x110F, 0x0100, 0x0017, 0x1002],
    attributes: &[
        (0x0000, &[0x0A, 0x00, 0x01, 0x00, 0x02]),
        (0x0001, &[0x35, 0x06, 0x19, 0x11, 0x0E, 0x19, 0x11, 0x0F]),
        (0x0004, &[0x35, 0x10, 0x35, 0x06, 0x19, 0x01, 0x00, 0x09, 0x00, 0x17, 0x35, 0x06, 0x19, 0x00, 0x17, 0x09, 0x01, 0x04]),
        (0x0005, &[0x35, 0x03, 0x19, 0x10, 0x02]),
        (0x0009, &[0x35, 0x08, 0x35, 0x06, 0x19, 0x11, 0x0E, 0x09, 0x01, 0x04]),
        (0x0311, &[0x09, 0x00, 0x02]),
    ],
};

pub const RECORDS: &[Record] = &[A2DP_SOURCE, AVRCP_CONTROLLER];

/// The attribute ID ranges of a request's list, as (first, last).
fn ranges(list: &[u8], out: &mut [(u16, u16); 16]) -> Option<usize> {
    let mut n = 0;
    let ok = each(list, |e| {
        if e.kind != UINT || n == out.len() { return false; }
        out[n] = match e.value.len() { 2 => (be16(e.value, 0), be16(e.value, 0)), 4 => (be16(e.value, 0), be16(e.value, 2)), _ => return false };
        n += 1;
        true
    });
    ok.then_some(n)
}

/// A record's attributes in the ranges, as one sequence element, into `out`; its length.
fn attribute_list(r: &Record, ranges: &[(u16, u16)], out: &mut [u8]) -> usize {
    let mut at = 3;
    for &(id, value) in r.attributes {
        if !ranges.iter().any(|&(a, b)| (a..=b).contains(&id)) { continue; }
        if at + 3 + value.len() > out.len() { break; }
        out[at..at + 3].copy_from_slice(&[0x09, (id >> 8) as u8, id as u8]);
        out[at + 3..at + 3 + value.len()].copy_from_slice(value);
        at += 3 + value.len();
    }
    let len = at - 3;
    out[..3].copy_from_slice(&[0x36, (len >> 8) as u8, len as u8]);
    at
}

fn matches(r: &Record, pattern: &[u32]) -> bool { pattern.iter().all(|u| r.uuids.contains(u)) }

pub struct Server { pub answered: u32 }

impl Default for Server { fn default() -> Self { Self::new() } }

impl Server {
    pub const fn new() -> Self { Self { answered: 0 } }

    /// One request PDU on an SDP channel; the answer goes back on it.
    pub fn pdu(&mut self, ctl: &mut Ctl, chans: &Channels, handle: u16, local: u16, p: &[u8]) {
        let mut out = [0u8; 512];
        let n = answer(p, &mut out);
        if n > 0 { self.answered += 1; chans.send(ctl, handle, local, &[&out[..n]]); }
    }
}

/// The response to request `p` into `out`; its length (0: nothing to answer).
pub fn answer(p: &[u8], out: &mut [u8; 512]) -> usize {
    if p.len() < 5 { return 0; }
    let (id, tid, plen) = (p[0], [p[1], p[2]], be16(p, 3) as usize);
    let error = |out: &mut [u8; 512], code: u16| { out[..7].copy_from_slice(&[ERROR, tid[0], tid[1], 0, 2, (code >> 8) as u8, code as u8]); 7 };
    let Some(params) = p.get(5..5 + plen) else { return error(out, INVALID_SYNTAX) };
    let mut body = [0u8; 480];
    // The search pattern: UUIDs, at most 12.
    let pattern = |b: &[u8], uuids: &mut [u32; 12]| -> Option<(usize, usize)> {
        let e = element(b).filter(|e| e.kind == SEQUENCE)?;
        let mut n = 0;
        let ok = each(e.value, |u| { match uuid32(&u) { Some(v) if n < 12 => { uuids[n] = v; n += 1; true } _ => false } });
        ok.then_some((n, e.len))
    };
    let (kind, total, rest) = match id {
        SEARCH_REQUEST => {
            let mut uuids = [0u32; 12];
            let Some((n, used)) = pattern(params, &mut uuids) else { return error(out, INVALID_SYNTAX) };
            let max = be16(params, used) as usize;
            let found: [Option<u32>; 4] = core::array::from_fn(|i| RECORDS.get(i).filter(|r| matches(r, &uuids[..n])).map(|r| r.handle));
            let handles = found.iter().flatten().take(max.max(1));
            let count = handles.clone().count();
            body[..4].copy_from_slice(&[0, count as u8, 0, count as u8]);
            for (i, h) in handles.enumerate() { body[4 + 4 * i..8 + 4 * i].copy_from_slice(&h.to_be_bytes()); }
            // No continuation: the handles always fit.
            body[4 + 4 * count] = 0;
            let len = 5 + 4 * count;
            out[..5].copy_from_slice(&[SEARCH_RESPONSE, tid[0], tid[1], (len >> 8) as u8, len as u8]);
            out[5..5 + len].copy_from_slice(&body[..len]);
            return 5 + len;
        }
        ATTRIBUTE_REQUEST if params.len() >= 6 => {
            let record = crate::buf::be32(params, 0);
            let max = be16(params, 4) as usize;
            let Some(r) = RECORDS.iter().find(|r| r.handle == record) else { return error(out, INVALID_HANDLE) };
            let Some(e) = element(&params[6..]).filter(|e| e.kind == SEQUENCE) else { return error(out, INVALID_SYNTAX) };
            let mut list = [(0u16, 0u16); 16];
            let Some(n) = ranges(e.value, &mut list) else { return error(out, INVALID_SYNTAX) };
            let total = attribute_list(r, &list[..n], &mut body);
            (ATTRIBUTE_RESPONSE, total, (max, &params[6 + e.len..]))
        }
        SEARCH_ATTRIBUTE_REQUEST => {
            let mut uuids = [0u32; 12];
            let Some((n, used)) = pattern(params, &mut uuids) else { return error(out, INVALID_SYNTAX) };
            let max = be16(params, used) as usize;
            let Some(e) = params.get(used + 2..).and_then(element).filter(|e| e.kind == SEQUENCE) else { return error(out, INVALID_SYNTAX) };
            let mut list = [(0u16, 0u16); 16];
            let Some(count) = ranges(e.value, &mut list) else { return error(out, INVALID_SYNTAX) };
            let mut at = 3;
            for r in RECORDS.iter().filter(|r| matches(r, &uuids[..n])) { at += attribute_list(r, &list[..count], &mut body[at..]); }
            let len = at - 3;
            body[..3].copy_from_slice(&[0x36, (len >> 8) as u8, len as u8]);
            (SEARCH_ATTRIBUTE_RESPONSE, at, (max, &params[used + 2 + e.len..]))
        }
        _ => return error(out, INVALID_SYNTAX),
    };
    // The bytes from the continuation's offset, at most the client's maximum; a continuation state for the rest.
    let (max, cont) = rest;
    let offset = match cont.first() { Some(0) | None => 0, Some(2) if cont.len() >= 3 => be16(cont, 1) as usize, _ => return error(out, INVALID_CONTINUATION) };
    if offset > total { return error(out, INVALID_CONTINUATION); }
    let n = (total - offset).min(max.max(7)).min(400);
    let more = offset + n < total;
    let len = 2 + n + if more { 3 } else { 1 };
    out[..7].copy_from_slice(&[kind, tid[0], tid[1], (len >> 8) as u8, len as u8, (n >> 8) as u8, n as u8]);
    out[7..7 + n].copy_from_slice(&body[offset..offset + n]);
    let end = offset + n;
    if more { out[7 + n..10 + n].copy_from_slice(&[2, (end >> 8) as u8, end as u8]); } else { out[7 + n] = 0; }
    5 + len
}
