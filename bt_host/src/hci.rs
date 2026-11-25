//! HCI commands and events (Bluetooth Core 5.3, Vol 4 Part E): what bt_hci sends and reads to bring a controller up
//! and hear the devices around. Over USB a command goes in a control transfer and an event comes in interrupt packets
//! (Vol 4 Part B), which `Events` joins.

pub const RESET: u16 = 0x0C03;
pub const SET_EVENT_MASK: u16 = 0x0C01;
pub const WRITE_INQUIRY_MODE: u16 = 0x0C45;
pub const READ_LOCAL_VERSION: u16 = 0x1001;
pub const READ_BD_ADDR: u16 = 0x1009;
pub const INQUIRY: u16 = 0x0401;
pub const LE_SET_EVENT_MASK: u16 = 0x2001;
pub const LE_SET_SCAN_PARAMETERS: u16 = 0x200B;
pub const LE_SET_SCAN_ENABLE: u16 = 0x200C;

pub const EV_INQUIRY_COMPLETE: u8 = 0x01;
pub const EV_COMMAND_COMPLETE: u8 = 0x0E;
pub const EV_COMMAND_STATUS: u8 = 0x0F;
pub const EV_INQUIRY_RESULT_RSSI: u8 = 0x22;
pub const EV_EXTENDED_INQUIRY_RESULT: u8 = 0x2F;
pub const EV_LE_META: u8 = 0x3E;
pub const LE_ADVERTISING_REPORT: u8 = 0x02;

/// The longest event: its code, its length byte and 255 bytes of parameters.
pub const EVENT_MAX: usize = 257;
/// The longest command: its opcode, its length byte and 255 bytes of parameters.
pub const COMMAND_MAX: usize = 258;

/// A command packet in `out`: the opcode, low byte first, the parameters' length, the parameters. Its length.
pub fn command(opcode: u16, params: &[u8], out: &mut [u8]) -> usize {
    let n = params.len().min(255);
    out[..3].copy_from_slice(&[opcode as u8, (opcode >> 8) as u8, n as u8]);
    out[3..3 + n].copy_from_slice(&params[..n]);
    3 + n
}

/// Joins an event's interrupt packets: a packet starts an event when none is open, and an event ends when its length
/// byte's parameters have come. A packet past that end, or a malformed start, is dropped and counted.
pub struct Events { buf: [u8; EVENT_MAX], len: usize, pub dropped: u32 }

impl Default for Events { fn default() -> Self { Self::new() } }

impl Events {
    pub const fn new() -> Self { Self { buf: [0; EVENT_MAX], len: 0, dropped: 0 } }

    /// Takes one packet; `each` gets every event it completes, code and parameters.
    pub fn feed(&mut self, packet: &[u8], mut each: impl FnMut(u8, &[u8])) {
        let mut rest = packet;
        while !rest.is_empty() {
            let want = if self.len < 2 { 2 } else { 2 + self.buf[1] as usize };
            let n = rest.len().min(want - self.len);
            self.buf[self.len..self.len + n].copy_from_slice(&rest[..n]);
            self.len += n;
            rest = &rest[n..];
            if self.len >= 2 && self.len == 2 + self.buf[1] as usize {
                each(self.buf[0], &self.buf[2..self.len]);
                self.len = 0;
                // A USB packet carries one event: what follows its end is not another's start.
                if !rest.is_empty() { self.dropped += 1; return; }
            }
        }
    }
}

/// A Command Complete event's parts: the command's opcode and its return parameters, status first.
pub fn complete(params: &[u8]) -> Option<(u16, &[u8])> {
    (params.len() >= 3).then(|| (u16::from_le_bytes([params[1], params[2]]), &params[3..]))
}

/// A Command Status event's parts: the status and the command's opcode.
pub fn status(params: &[u8]) -> Option<(u8, u16)> {
    (params.len() >= 4).then(|| (params[0], u16::from_le_bytes([params[2], params[3]])))
}

/// Read Local Version Information's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version { pub hci: u8, pub hci_rev: u16, pub lmp: u8, pub manufacturer: u16, pub lmp_subver: u16 }

impl Version {
    pub fn parse(ret: &[u8]) -> Option<Self> {
        if ret.len() < 9 || ret[0] != 0 { return None; }
        let w = |i: usize| u16::from_le_bytes([ret[i], ret[i + 1]]);
        Some(Self { hci: ret[1], hci_rev: w(2), lmp: ret[4], manufacturer: w(5), lmp_subver: w(7) })
    }
}

/// Read BD_ADDR's answer: the address as written, most significant byte first.
pub fn bd_addr(ret: &[u8]) -> Option<[u8; 6]> {
    if ret.len() < 7 || ret[0] != 0 { return None; }
    Some(core::array::from_fn(|i| ret[6 - i]))
}

/// A device heard: its address (most significant byte first), whether over LE, its signal, its name if it gave one,
/// and its class of device (BR/EDR) or appearance (LE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heard<'a> { pub addr: [u8; 6], pub le: bool, pub rssi: i8, pub name: Option<&'a [u8]>, pub kind: u32 }

fn addr_at(b: &[u8]) -> [u8; 6] { core::array::from_fn(|i| b[5 - i]) }

/// The name and the appearance in advertising or extended inquiry data (Core Supplement Part A, 1.2 and 1.12): the
/// complete name, else the shortened one. Malformed data ends the walk.
pub fn ad_fields(mut data: &[u8]) -> (Option<&[u8]>, Option<u16>) {
    let (mut name, mut short, mut appearance) = (None, None, None);
    while let [len, rest @ ..] = data {
        let len = *len as usize;
        if len == 0 || len > rest.len() { break; }
        let (kind, value) = (rest[0], &rest[1..len]);
        match kind {
            0x09 => name = Some(value),
            0x08 => short = Some(value),
            0x19 if value.len() == 2 => appearance = Some(u16::from_le_bytes([value[0], value[1]])),
            _ => {}
        }
        data = &rest[len..];
    }
    (name.or(short), appearance)
}

/// The devices in an LE Advertising Report's parameters (after the subevent code).
pub fn le_reports(params: &[u8], mut each: impl FnMut(Heard)) {
    let Some((&count, mut rest)) = params.split_first() else { return };
    for _ in 0..count {
        // Event type, address type, address, data length.
        if rest.len() < 9 { return; }
        let (addr, len) = (addr_at(&rest[2..8]), rest[8] as usize);
        if rest.len() < 9 + len + 1 { return; }
        let (data, rssi) = (&rest[9..9 + len], rest[9 + len] as i8);
        let (name, appearance) = ad_fields(data);
        each(Heard { addr, le: true, rssi, name, kind: appearance.unwrap_or(0) as u32 });
        rest = &rest[10 + len..];
    }
}

/// The device in an Extended Inquiry Result's parameters.
pub fn extended_inquiry(params: &[u8]) -> Option<Heard<'_>> {
    // Count (1), address, page scan repetition mode, reserved, class (3), clock offset (2), RSSI, 240 bytes of EIR.
    if params.len() < 15 || params[0] != 1 { return None; }
    let class = u32::from_le_bytes([params[9], params[10], params[11], 0]);
    let (name, _) = ad_fields(&params[15..]);
    Some(Heard { addr: addr_at(&params[1..7]), le: false, rssi: params[14] as i8, name, kind: class })
}

/// The devices in an Inquiry Result with RSSI's parameters.
pub fn inquiry_rssi(params: &[u8], mut each: impl FnMut(Heard)) {
    let Some((&count, rest)) = params.split_first() else { return };
    for i in 0..count as usize {
        // Address, page scan repetition mode, reserved, class (3), clock offset (2), RSSI: 14 bytes each.
        let Some(r) = rest.get(i * 14..i * 14 + 14) else { return };
        each(Heard { addr: addr_at(&r[..6]), le: false, rssi: r[13] as i8, name: None, kind: u32::from_le_bytes([r[8], r[9], r[10], 0]) });
    }
}

/// A class of device's major class (Assigned Numbers 2.8), for the log.
pub fn major_class(class: u32) -> &'static str {
    match class >> 8 & 0x1F {
        1 => "COMPUTER", 2 => "PHONE", 3 => "NETWORK", 4 => "AUDIO", 5 => "PERIPHERAL", 6 => "IMAGING", 7 => "WEARABLE", 8 => "TOY", 9 => "HEALTH",
        _ => "OTHER",
    }
}
