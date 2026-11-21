//! A monitor's EDID (213-DRV-0086): the base block (VESA E-EDID 1.3/1.4) and the CTA-861 extension, for what a
//! display driver chooses a mode by: the detailed timings, the range limits, and whether the sink is HDMI and up to
//! which TMDS clock. Fixed-size, no allocation; `tests/display_host.rs` checks it.

use crate::timing::Timing;

pub const BLOCK: usize = 128;
/// Detailed timings kept: four in the base block, the rest from the CTA extension.
pub const MAX_TIMINGS: usize = 10;
/// Short video descriptors (CTA VICs) kept.
pub const MAX_VICS: usize = 32;

const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
const CTA_TAG: u8 = 0x02;
const OUI_HDMI: u32 = 0x000C03; // HDMI Licensing: the HDMI 1.4 vendor-specific data block
const OUI_HDMI_FORUM: u32 = 0xC45DD8; // HDMI Forum: the HF-VSDB of HDMI 2.0

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Fewer bytes than the base block.
    Short,
    /// The base block does not start with the EDID header.
    Header,
    /// A block whose bytes do not sum to 0 modulo 256 (the block's index).
    Checksum(u8),
    /// Not EDID 1.x.
    Version,
}

/// The range limits descriptor (tag 0xFD).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Range {
    pub min_v_hz: u16,
    pub max_v_hz: u16,
    pub min_h_khz: u16,
    pub max_h_khz: u16,
    /// The largest pixel clock, in MHz (a multiple of 10).
    pub max_pixel_mhz: u16,
}

/// What the HDMI data blocks of a CTA extension say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hdmi {
    /// The source physical address A.B.C.D as 0xABCD.
    pub physical_address: u16,
    /// The HDMI 1.4 block's Max_TMDS_Clock, in MHz; 0 when not stated.
    pub max_tmds_mhz: u16,
    /// The HDMI Forum block's Max_TMDS_Character_Rate, in MHz; 0 when absent or not above 340.
    pub forum_max_tmds_mhz: u16,
    /// Deep colour: 10, 12 and 16 bits a colour.
    pub deep_30: bool,
    pub deep_36: bool,
    pub deep_48: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edid {
    /// The three-letter manufacturer code (`DEL`, `SAM`, ...).
    pub manufacturer: [u8; 3],
    pub product: u16,
    pub year: u16,
    pub version: u8,
    pub revision: u8,
    pub digital: bool,
    /// Bits a colour of a digital input (EDID 1.4); 0 when undefined.
    pub bits_per_color: u8,
    pub width_cm: u8,
    pub height_cm: u8,
    /// The monitor's name descriptor (tag 0xFC), up to 13 characters.
    pub name: [u8; 13],
    pub name_len: usize,
    pub range: Option<Range>,
    /// Detailed timings in EDID order: the first is the preferred one.
    pub timings: [Option<Timing>; MAX_TIMINGS],
    pub timing_count: usize,
    /// The extension blocks the base block announces.
    pub extensions: u8,
    /// Whether a CTA-861 extension was read.
    pub cta: bool,
    /// The CTA extension's video identification codes, native ones included.
    pub vics: [u8; MAX_VICS],
    pub vic_count: usize,
    /// The HDMI vendor-specific data block, if the sink has one: without it the sink takes DVI signalling only.
    pub hdmi: Option<Hdmi>,
}

/// One 18-byte detailed timing descriptor; None for a display descriptor (a zero pixel clock) or a short slice.
pub fn detailed_timing(d: &[u8]) -> Option<Timing> {
    if d.len() < 18 { return None; }
    let clock = u16::from_le_bytes([d[0], d[1]]) as u32;
    if clock == 0 { return None; }
    let hi = |low: u8, high: u8| low as u16 | (high as u16) << 8;
    let h_active = hi(d[2], d[4] >> 4);
    let h_blank = hi(d[3], d[4] & 0xF);
    let v_active = hi(d[5], d[7] >> 4);
    let v_blank = hi(d[6], d[7] & 0xF);
    let h_front = hi(d[8], (d[11] >> 6) & 3);
    let h_sync = hi(d[9], (d[11] >> 4) & 3);
    let v_front = (d[10] >> 4) as u16 | (((d[11] >> 2) & 3) as u16) << 4;
    let v_sync = (d[10] & 0xF) as u16 | ((d[11] & 3) as u16) << 4;
    let flags = d[17];
    // Digital separate sync (bits 4:3 = 11) gives both polarities in bits 2 and 1; other kinds give hsync's in bit 1.
    let separate = flags & 0x18 == 0x18;
    Some(Timing {
        pixel_khz: clock * 10,
        h_active,
        h_front,
        h_sync,
        h_back: h_blank.checked_sub(h_front + h_sync)?,
        v_active,
        v_front,
        v_sync,
        v_back: v_blank.checked_sub(v_front + v_sync)?,
        h_sync_positive: flags & 0x02 != 0,
        v_sync_positive: separate && flags & 0x04 != 0,
        interlaced: flags & 0x80 != 0,
    })
}

fn checksum_ok(block: &[u8]) -> bool { block.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) == 0 }

impl Edid {
    /// The base block and, if present, the first extension when it is CTA-861.
    pub fn parse(bytes: &[u8]) -> Result<Edid, Error> {
        if bytes.len() < BLOCK { return Err(Error::Short); }
        let base = &bytes[..BLOCK];
        if base[..8] != HEADER { return Err(Error::Header); }
        if !checksum_ok(base) { return Err(Error::Checksum(0)); }
        if base[18] != 1 { return Err(Error::Version); }
        let id = u16::from_be_bytes([base[8], base[9]]);
        let letter = |shift: u16| b'A' - 1 + ((id >> shift) & 0x1F) as u8;
        let digital = base[20] & 0x80 != 0;
        let mut edid = Edid {
            manufacturer: [letter(10), letter(5), letter(0)],
            product: u16::from_le_bytes([base[10], base[11]]),
            year: 1990 + base[17] as u16,
            version: base[18],
            revision: base[19],
            digital,
            bits_per_color: if digital && base[19] >= 4 { match (base[20] >> 4) & 7 { 1 => 6, 2 => 8, 3 => 10, 4 => 12, 5 => 14, 6 => 16, _ => 0 } } else { 0 },
            width_cm: base[21],
            height_cm: base[22],
            name: [0; 13],
            name_len: 0,
            range: None,
            timings: [None; MAX_TIMINGS],
            timing_count: 0,
            extensions: base[126],
            cta: false,
            vics: [0; MAX_VICS],
            vic_count: 0,
            hdmi: None,
        };
        for d in base[54..126].chunks(18) {
            if let Some(t) = detailed_timing(d) { edid.push(t); continue; }
            match d[3] {
                0xFC => {
                    let text = &d[5..18];
                    let n = text.iter().position(|&c| c == b'\n').unwrap_or(text.len());
                    edid.name[..n].copy_from_slice(&text[..n]);
                    edid.name_len = n;
                    while edid.name_len > 0 && edid.name[edid.name_len - 1] == b' ' { edid.name_len -= 1; }
                }
                0xFD => {
                    // Bits 0-3 of byte 4 add 255 to the minimum (bits 0, 2) or maximum (bits 1, 3) rates (EDID 1.4).
                    let plus = |bit: u8| if d[4] & 1 << bit != 0 { 255 } else { 0 };
                    edid.range = Some(Range {
                        min_v_hz: d[5] as u16 + plus(0), max_v_hz: d[6] as u16 + plus(1),
                        min_h_khz: d[7] as u16 + plus(2), max_h_khz: d[8] as u16 + plus(3),
                        max_pixel_mhz: d[9] as u16 * 10,
                    });
                }
                _ => {}
            }
        }
        if edid.extensions > 0 && bytes.len() >= 2 * BLOCK {
            let ext = &bytes[BLOCK..2 * BLOCK];
            if ext[0] == CTA_TAG {
                if !checksum_ok(ext) { return Err(Error::Checksum(1)); }
                edid.read_cta(ext);
            }
        }
        Ok(edid)
    }

    fn push(&mut self, t: Timing) {
        if self.timing_count < MAX_TIMINGS { self.timings[self.timing_count] = Some(t); self.timing_count += 1; }
    }

    fn read_cta(&mut self, ext: &[u8]) {
        self.cta = true;
        let dtd_start = (ext[2] as usize).min(BLOCK - 1);
        let mut at = 4;
        while dtd_start >= 4 && at < dtd_start {
            let tag = ext[at] >> 5;
            let len = (ext[at] & 0x1F) as usize;
            let body = &ext[at + 1..(at + 1 + len).min(dtd_start)];
            match tag {
                2 => for &svd in body {
                    // VICs 129-192 are 1-64 marked native; others are the code itself (CTA-861-F).
                    let vic = if (129..=192).contains(&svd) { svd & 0x7F } else { svd };
                    if self.vic_count < MAX_VICS { self.vics[self.vic_count] = vic; self.vic_count += 1; }
                },
                3 if body.len() >= 5 => {
                    let oui = body[0] as u32 | (body[1] as u32) << 8 | (body[2] as u32) << 16;
                    if oui == OUI_HDMI {
                        let hdmi = self.hdmi.get_or_insert(Hdmi::default());
                        hdmi.physical_address = u16::from_be_bytes([body[3], body[4]]);
                        if let Some(&flags) = body.get(5) {
                            hdmi.deep_48 = flags & 0x40 != 0;
                            hdmi.deep_36 = flags & 0x20 != 0;
                            hdmi.deep_30 = flags & 0x10 != 0;
                        }
                        if let Some(&clock) = body.get(6) { hdmi.max_tmds_mhz = clock as u16 * 5; }
                    } else if oui == OUI_HDMI_FORUM {
                        // Only alongside the HDMI 1.4 block, which a sink must have to be HDMI at all.
                        if let Some(hdmi) = self.hdmi.as_mut() { hdmi.forum_max_tmds_mhz = body[4] as u16 * 5; }
                    }
                }
                _ => {}
            }
            at += 1 + len;
        }
        if dtd_start >= 4 {
            let mut d = dtd_start;
            while d + 18 <= BLOCK - 1 {
                match detailed_timing(&ext[d..d + 18]) { Some(t) => self.push(t), None => break }
                d += 18;
            }
        }
    }

    /// The preferred timing: the first detailed one (EDID 1.3 and later).
    pub fn preferred(&self) -> Option<Timing> { self.timings[0] }

    /// The detailed timings in order.
    pub fn timings(&self) -> impl Iterator<Item = Timing> + '_ { self.timings[..self.timing_count].iter().flatten().copied() }

    /// The monitor's name, as ASCII.
    pub fn name(&self) -> &[u8] { &self.name[..self.name_len] }

    /// The largest TMDS clock the sink states, in kHz: the HDMI Forum's, else HDMI 1.4's, else the range limit's,
    /// else DVI's single link.
    pub fn max_tmds_khz(&self) -> u32 {
        let hdmi = self.hdmi.unwrap_or_default();
        let mhz = [hdmi.forum_max_tmds_mhz, hdmi.max_tmds_mhz, self.range.map_or(0, |r| r.max_pixel_mhz)].into_iter().find(|&m| m != 0);
        mhz.map_or(crate::timing::DVI_SINGLE_LINK_KHZ, |m| m as u32 * 1000)
    }

    /// The largest detailed timing, by area then refresh, whose 8-bit TMDS clock is within both `limit_khz` and the
    /// sink's own limit.
    pub fn best_within(&self, limit_khz: u32) -> Option<Timing> {
        let limit = limit_khz.min(self.max_tmds_khz());
        self.timings().filter(|t| !t.interlaced && t.pixel_khz <= limit && t.check().is_ok())
            .max_by_key(|t| (t.h_active as u32 * t.v_active as u32, t.refresh_millihertz()))
    }
}
