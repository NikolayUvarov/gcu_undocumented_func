//! 802.11 frames as the station reads and writes them (IEEE 802.11-2020 9.2-9.4): MAC headers, the management frames
//! of joining a network, elements, and data frames carrying Ethernet payloads behind an LLC/SNAP header.
use crate::Error;

pub type Mac = [u8; 6];
pub const BROADCAST: Mac = [0xFF; 6];

/// Frame control, first byte: type (bits 2-3) and subtype (bits 4-7).
pub const TYPE_MANAGEMENT: u8 = 0;
pub const TYPE_DATA: u8 = 2;
/// Management subtypes.
pub const ASSOCIATION_REQUEST: u8 = 0;
pub const ASSOCIATION_RESPONSE: u8 = 1;
pub const PROBE_REQUEST: u8 = 4;
pub const PROBE_RESPONSE: u8 = 5;
pub const BEACON: u8 = 8;
pub const DISASSOCIATION: u8 = 10;
pub const AUTHENTICATION: u8 = 11;
pub const DEAUTHENTICATION: u8 = 12;
/// Frame control, second byte.
pub const TO_DS: u8 = 0x01;
pub const FROM_DS: u8 = 0x02;
pub const RETRY: u8 = 0x08;
pub const PROTECTED: u8 = 0x40;

/// Element IDs.
pub const SSID: u8 = 0;
pub const RATES: u8 = 1;
pub const DS_PARAMETER: u8 = 3;
pub const RSN: u8 = 48;
pub const EXTENDED_RATES: u8 = 50;

/// Capability information: an ESS, privacy required, short preamble, short slot time.
pub const CAPABILITY_ESS: u16 = 0x0001;
pub const CAPABILITY_PRIVACY: u16 = 0x0010;
pub const CAPABILITY_SHORT_PREAMBLE: u16 = 0x0020;
pub const CAPABILITY_SHORT_SLOT: u16 = 0x0400;
/// An association refused because the station lacks one of the network's basic rates (802.11 status 18).
pub const STATUS_BASIC_RATES: u16 = 18;

/// The LLC/SNAP header before an Ethernet type in a data frame's body (RFC 1042).
pub const SNAP: [u8; 6] = [0xAA, 0xAA, 0x03, 0x00, 0x00, 0x00];
pub const ETHERTYPE_EAPOL: u16 = 0x888E;

/// A MAC header's fields; `len` is where the body starts (the CCMP header, if protected).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub kind: u8,
    pub subtype: u8,
    pub flags: u8,
    pub a1: Mac,
    pub a2: Mac,
    pub a3: Mac,
    pub a4: Option<Mac>,
    pub sequence: u16,
    pub tid: Option<u8>,
    pub len: usize,
}

fn mac(bytes: &[u8]) -> Mac { let mut m = [0u8; 6]; m.copy_from_slice(&bytes[..6]); m }

/// Reads the MAC header of a management or data frame (control frames are not passed to the station).
pub fn header(frame: &[u8]) -> Result<Header, Error> {
    if frame.len() < 24 || frame[0] & 3 != 0 { return Err(Error::Malformed); }
    let (kind, subtype, flags) = ((frame[0] >> 2) & 3, frame[0] >> 4, frame[1]);
    if kind != TYPE_MANAGEMENT && kind != TYPE_DATA { return Err(Error::Malformed); }
    let mut h = Header { kind, subtype, flags, a1: mac(&frame[4..]), a2: mac(&frame[10..]), a3: mac(&frame[16..]), a4: None,
                         sequence: u16::from_le_bytes([frame[22], frame[23]]) >> 4, tid: None, len: 24 };
    if kind == TYPE_DATA && flags & (TO_DS | FROM_DS) == TO_DS | FROM_DS {
        h.a4 = Some(mac(frame.get(24..30).ok_or(Error::Malformed)?));
        h.len = 30;
    }
    if kind == TYPE_DATA && subtype & 8 != 0 {
        h.tid = Some(frame.get(h.len).ok_or(Error::Malformed)? & 0x0F);
        h.len += 2 + if flags & 0x80 != 0 { 4 } else { 0 };
    }
    if frame.len() < h.len { return Err(Error::Malformed); }
    Ok(h)
}

/// The elements of a management frame's body, in order; a truncated last element ends them.
pub struct Elements<'a> { data: &'a [u8] }

impl<'a> Elements<'a> {
    pub fn new(data: &'a [u8]) -> Self { Elements { data } }
    /// The first element with `id`.
    pub fn find(data: &'a [u8], id: u8) -> Option<&'a [u8]> { Elements::new(data).find(|(i, _)| *i == id).map(|(_, body)| body) }
    /// The first element with `id`, header included (for the RSN element, which handshakes compare whole).
    pub fn find_whole(data: &'a [u8], id: u8) -> Option<&'a [u8]> {
        let mut at = 0;
        while at + 2 <= data.len() {
            let end = at + 2 + data[at + 1] as usize;
            if end > data.len() { return None; }
            if data[at] == id { return Some(&data[at..end]); }
            at = end;
        }
        None
    }
}

impl<'a> Iterator for Elements<'a> {
    type Item = (u8, &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        let (&id, rest) = self.data.split_first()?;
        let (&len, rest) = rest.split_first()?;
        let body = rest.get(..len as usize)?;
        self.data = &rest[len as usize..];
        Some((id, body))
    }
}

/// Writes bytes at a cursor, refusing to run past the buffer.
pub struct Writer<'a> { out: &'a mut [u8], at: usize }

impl<'a> Writer<'a> {
    pub fn new(out: &'a mut [u8]) -> Self { Writer { out, at: 0 } }
    pub fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self.at + bytes.len();
        self.out.get_mut(self.at..end).ok_or(Error::Space)?.copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }
    pub fn element(&mut self, id: u8, body: &[u8]) -> Result<(), Error> {
        if body.len() > 255 { return Err(Error::Malformed); }
        self.put(&[id, body.len() as u8])?;
        self.put(body)
    }
    pub fn len(&self) -> usize { self.at }
}

/// A management frame's header: to `a1`, from `own`, in network `bssid`, sequence number `sequence`.
pub fn management(w: &mut Writer, subtype: u8, a1: &Mac, own: &Mac, bssid: &Mac, sequence: u16, duration: u16) -> Result<(), Error> {
    w.put(&[subtype << 4 | TYPE_MANAGEMENT << 2, 0])?;
    w.put(&duration.to_le_bytes())?;
    w.put(a1)?;
    w.put(own)?;
    w.put(bssid)?;
    w.put(&((sequence & 0x0FFF) << 4).to_le_bytes())
}

/// The rates the station offers, in 500 kb/s units with the basic-rate bit where the band requires it: on 2.4 GHz the
/// DSSS rates are basic and the OFDM ones follow when the radio has OFDM there (`ofdm`; without it, 802.11b's four
/// alone); on 5 GHz only OFDM, 6, 12 and 24 Mb/s basic.
pub fn rates(channel: u8, ofdm: bool) -> (&'static [u8], &'static [u8]) {
    if channel > 14 {
        (&[0x8C, 0x12, 0x98, 0x24, 0xB0, 0x48, 0x60, 0x6C], &[])
    } else if ofdm {
        (&[0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24], &[0x30, 0x48, 0x60, 0x6C])
    } else {
        (&[0x82, 0x84, 0x8B, 0x96], &[])
    }
}

fn put_rates(w: &mut Writer, channel: u8, ofdm: bool) -> Result<(), Error> {
    let (rates, extended) = rates(channel, ofdm);
    w.element(RATES, rates)?;
    if !extended.is_empty() { w.element(EXTENDED_RATES, extended)?; }
    Ok(())
}

/// A probe request for `ssid` (empty: any network), broadcast.
pub fn probe_request(out: &mut [u8], own: &Mac, ssid: &[u8], channel: u8, ofdm: bool, sequence: u16) -> Result<usize, Error> {
    let mut w = Writer::new(out);
    management(&mut w, PROBE_REQUEST, &BROADCAST, own, &BROADCAST, sequence, 0)?;
    w.element(SSID, ssid)?;
    put_rates(&mut w, channel, ofdm)?;
    Ok(w.len())
}

/// Open System authentication, the station's first frame (algorithm 0, transaction 1, status 0).
pub fn authentication(out: &mut [u8], own: &Mac, bssid: &Mac, sequence: u16, duration: u16) -> Result<usize, Error> {
    let mut w = Writer::new(out);
    management(&mut w, AUTHENTICATION, bssid, own, bssid, sequence, duration)?;
    w.put(&[0, 0, 1, 0, 0, 0])?;
    Ok(w.len())
}

/// An association request for an ESS: its capability, a listen interval of 10 beacons, the SSID, the rates, and the
/// station's RSN element when the network is protected.
pub fn association_request(out: &mut [u8], own: &Mac, bssid: &Mac, capability: u16, ssid: &[u8], channel: u8, ofdm: bool,
                           rsne: Option<&[u8]>, sequence: u16, duration: u16) -> Result<usize, Error> {
    let mut w = Writer::new(out);
    management(&mut w, ASSOCIATION_REQUEST, bssid, own, bssid, sequence, duration)?;
    w.put(&capability.to_le_bytes())?;
    w.put(&10u16.to_le_bytes())?;
    w.element(SSID, ssid)?;
    put_rates(&mut w, channel, ofdm)?;
    if let Some(rsne) = rsne { w.put(rsne)?; }
    Ok(w.len())
}

/// A deauthentication with `reason` (3: the station is leaving).
pub fn deauthentication(out: &mut [u8], own: &Mac, bssid: &Mac, reason: u16, sequence: u16) -> Result<usize, Error> {
    let mut w = Writer::new(out);
    management(&mut w, DEAUTHENTICATION, bssid, own, bssid, sequence, 0)?;
    w.put(&reason.to_le_bytes())?;
    Ok(w.len())
}

/// A data frame to the distribution system carrying an Ethernet payload: A1 the BSSID, A2 the station, A3 the
/// destination; the body is LLC/SNAP, the Ethernet type and the payload. Not yet protected.
pub fn data(out: &mut [u8], own: &Mac, bssid: &Mac, destination: &Mac, ethertype: u16, payload: &[u8], sequence: u16,
            duration: u16) -> Result<usize, Error> {
    let mut w = Writer::new(out);
    w.put(&[TYPE_DATA << 2, TO_DS])?;
    w.put(&duration.to_le_bytes())?;
    w.put(bssid)?;
    w.put(own)?;
    w.put(destination)?;
    w.put(&((sequence & 0x0FFF) << 4).to_le_bytes())?;
    w.put(&SNAP)?;
    w.put(&ethertype.to_be_bytes())?;
    w.put(payload)?;
    Ok(w.len())
}

/// The Ethernet frame a received (decrypted) data frame from the distribution system carries: destination A1, source
/// A3, then the type and payload behind LLC/SNAP. Written into `out`; returns its length and the Ethernet type.
pub fn to_ethernet(frame: &[u8], h: &Header, out: &mut [u8]) -> Result<(usize, u16), Error> {
    if h.kind != TYPE_DATA || h.flags & (TO_DS | FROM_DS) != FROM_DS || h.subtype & 4 != 0 { return Err(Error::Unexpected); }
    let body = &frame[h.len..];
    if body.len() < 8 || body[..6] != SNAP { return Err(Error::Malformed); }
    let ethertype = u16::from_be_bytes([body[6], body[7]]);
    let payload = &body[8..];
    let len = 14 + payload.len();
    let out = out.get_mut(..len).ok_or(Error::Space)?;
    out[..6].copy_from_slice(&h.a1);
    out[6..12].copy_from_slice(&h.a3);
    out[12..14].copy_from_slice(&ethertype.to_be_bytes());
    out[14..].copy_from_slice(payload);
    Ok((len, ethertype))
}

/// Whether an address is a group (multicast or broadcast) address.
pub fn group(address: &Mac) -> bool { address[0] & 1 != 0 }
