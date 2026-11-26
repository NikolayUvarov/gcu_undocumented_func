//! HCI ACL data (Core 5.3 Vol 4 Part E 5.4.2): the header, the bulk IN endpoint's packets joined into ACL packets
//! (Vol 4 Part B 2.1), and L2CAP PDUs reassembled from an ACL link's fragments (Vol 3 Part A 7.2).

/// Handle (12 bits), packet boundary flag (2), broadcast flag (2), then the data's length.
pub const HEADER: usize = 4;
/// Packet boundary flags: a first fragment the controller may not flush (host to controller, LE and BR/EDR), a
/// continuing one, and a first flushable one (BR/EDR host to controller; every first fragment controller to host).
pub const PB_FIRST_NO_FLUSH: u8 = 0b00;
pub const PB_CONTINUING: u8 = 0b01;
pub const PB_FIRST: u8 = 0b10;
/// The longest ACL packet bt_host takes from the controller: its data length is read from it and is ~1 KB.
pub const PACKET_MAX: usize = 2048;
/// The longest L2CAP PDU it reassembles: its header and the MTU it asks for (l2cap::MTU) with room for options.
pub const PDU_MAX: usize = 1700;

pub fn header(handle: u16, pb: u8, len: usize) -> [u8; HEADER] {
    let word = handle & 0x0FFF | (pb as u16 & 3) << 12;
    [word as u8, (word >> 8) as u8, len as u8, (len >> 8) as u8]
}

/// An ACL packet's handle, packet boundary flag and data; None when it is short or its length is wrong.
pub fn parse(packet: &[u8]) -> Option<(u16, u8, &[u8])> {
    if packet.len() < HEADER { return None; }
    let word = u16::from_le_bytes([packet[0], packet[1]]);
    let len = u16::from_le_bytes([packet[2], packet[3]]) as usize;
    let data = packet.get(HEADER..HEADER + len)?;
    Some((word & 0x0FFF, (word >> 12) as u8 & 3, data))
}

/// Joins the bulk IN endpoint's USB packets into ACL packets by their headers' lengths. A packet too long for the
/// buffer is skipped to the end of its USB transfer (the first packet shorter than the endpoint's), and counted.
pub struct Stream { buf: [u8; PACKET_MAX], len: usize, skipping: bool, packet: usize, pub dropped: u32 }

impl Stream {
    pub const fn new(packet: usize) -> Self { Self { buf: [0; PACKET_MAX], len: 0, skipping: false, packet, dropped: 0 } }

    /// One USB packet; `each` gets every ACL packet it completes.
    pub fn feed(&mut self, usb: &[u8], mut each: impl FnMut(&[u8])) {
        if self.skipping {
            if usb.len() < self.packet { self.skipping = false; }
            return;
        }
        let mut rest = usb;
        while !rest.is_empty() {
            let want = if self.len < HEADER { HEADER } else { HEADER + u16::from_le_bytes([self.buf[2], self.buf[3]]) as usize };
            if want > PACKET_MAX {
                self.dropped += 1;
                self.len = 0;
                self.skipping = usb.len() >= self.packet;
                return;
            }
            let n = rest.len().min(want - self.len);
            self.buf[self.len..self.len + n].copy_from_slice(&rest[..n]);
            self.len += n;
            rest = &rest[n..];
            if self.len >= HEADER && self.len == HEADER + u16::from_le_bytes([self.buf[2], self.buf[3]]) as usize {
                each(&self.buf[..self.len]);
                self.len = 0;
            }
        }
    }
}

/// One link's L2CAP PDU being reassembled from its ACL fragments.
pub struct Pdu { buf: [u8; PDU_MAX], len: usize, want: usize, pub dropped: u32 }

impl Default for Pdu { fn default() -> Self { Self::new() } }

impl Pdu {
    pub const fn new() -> Self { Self { buf: [0; PDU_MAX], len: 0, want: 0, dropped: 0 } }

    /// One ACL packet's data and whether it starts a PDU; the whole PDU, its basic header included, once complete.
    /// A start before the last PDU ended, a continuation without a start, or a PDU too long, is dropped and counted.
    pub fn feed(&mut self, first: bool, data: &[u8]) -> Option<&[u8]> {
        if first {
            if self.want != 0 { self.dropped += 1; }
            (self.len, self.want) = (0, 0);
            if data.len() < 2 { self.dropped += 1; return None; }
            let want = 4 + u16::from_le_bytes([data[0], data[1]]) as usize;
            if want > PDU_MAX { self.dropped += 1; return None; }
            self.want = want;
        } else if self.want == 0 {
            self.dropped += 1;
            return None;
        }
        let n = data.len().min(self.want - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&data[..n]);
        self.len += n;
        if self.len < self.want { return None; }
        let len = self.len;
        (self.len, self.want) = (0, 0);
        Some(&self.buf[..len])
    }

    pub fn reset(&mut self) { (self.len, self.want) = (0, 0); }
}
