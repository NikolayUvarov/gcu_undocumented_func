//! Baseline JPEG pictures (000-APP-0050's backgrounds, a camera's Motion JPEG frames, 158-DRV-0084) decoded a row of
//! blocks at a time: 8-bit greyscale or YCbCr, any sampling factors, Huffman coding with restart markers. Rows go to a
//! sink as `0x00RRGGBB` pixels as each row of blocks is done, so a large photo is never held whole. Progressive and
//! 12-bit pictures are refused. A picture without Huffman tables takes the standard ones (JPEG Annex K.3), as a UVC
//! camera's frames may (USB Video Class, Motion-JPEG payload). The input may be a device's: every length and value is
//! bounded, and a picture whose coded data runs out is refused (`CUT_SHORT`), never padded out. No system calls:
//! host-tested and fuzzed in tests/image_host.rs.
use crate::jpeg::{AC_CHROMA, AC_LUMA, DC_CHROMA, DC_LUMA, ZIGZAG};
use alloc::vec;
use alloc::vec::Vec;

pub type Error = &'static str;

/// A picture whose coded data ran out before its last block: a frame cut short.
pub const CUT_SHORT: Error = "a JPEG cut short";

// The inverse DCT as libjpeg's jidctint computes it (Loeffler, Ligtenberg and Moschytz's 8-point transform, the inverse
// of jpeg.rs's): columns, then rows, in 64-bit integers so that no bounded coefficient overflows.
const CONST_BITS: u32 = 13;
const PASS1_BITS: u32 = 2;
const F0_298: i64 = 2446; // 0.298631336 × 2^13, and so on
const F0_390: i64 = 3196;
const F0_541: i64 = 4433;
const F0_765: i64 = 6270;
const F0_899: i64 = 7373;
const F1_175: i64 = 9633;
const F1_501: i64 = 12299;
const F1_847: i64 = 15137;
const F1_961: i64 = 16069;
const F2_053: i64 = 16819;
const F2_562: i64 = 20995;
const F3_072: i64 = 25172;

// The 8-point transform of `i`, its results 2^CONST_BITS times too large.
fn idct8(i: [i64; 8]) -> [i64; 8] {
    let z1 = (i[2] + i[6]) * F0_541;
    let (e2, e3) = (z1 - i[6] * F1_847, z1 + i[2] * F0_765);
    let (e0, e1) = ((i[0] + i[4]) << CONST_BITS, (i[0] - i[4]) << CONST_BITS);
    let (t10, t13, t11, t12) = (e0 + e3, e0 - e3, e1 + e2, e1 - e2);
    let (o0, o1, o2, o3) = (i[7], i[5], i[3], i[1]);
    let z5 = (o0 + o2 + o1 + o3) * F1_175;
    let (z1, z2) = (-(o0 + o3) * F0_899, -(o1 + o2) * F2_562);
    let (z3, z4) = (z5 - (o0 + o2) * F1_961, z5 - (o1 + o3) * F0_390);
    let (o0, o1, o2, o3) = (o0 * F0_298 + z1 + z3, o1 * F2_053 + z2 + z4, o2 * F3_072 + z2 + z3, o3 * F1_501 + z1 + z4);
    [t10 + o3, t11 + o2, t12 + o1, t13 + o0, t13 - o0, t12 - o1, t11 - o2, t10 - o3]
}

fn descale(x: i64, n: u32) -> i64 { (x + (1 << (n - 1))) >> n }

// 8 × 8 coefficients (natural order, dequantized) into samples, 0..=255. `flat`: no AC coefficient.
fn idct(block: &[i32; 64], flat: bool, out: &mut [u8; 64]) {
    if flat {
        out.fill((((block[0] as i64 + 4) >> 3) + 128).clamp(0, 255) as u8);
        return;
    }
    let mut work = [0i64; 64];
    for x in 0..8 {
        let column: [i64; 8] = core::array::from_fn(|y| block[y * 8 + x] as i64);
        if column[1..].iter().all(|&c| c == 0) {
            for y in 0..8 { work[y * 8 + x] = column[0] << PASS1_BITS; }
            continue;
        }
        let t = idct8(column);
        for y in 0..8 { work[y * 8 + x] = descale(t[y], CONST_BITS - PASS1_BITS); }
    }
    for y in 0..8 {
        let t = idct8(core::array::from_fn(|x| work[y * 8 + x]));
        for x in 0..8 { out[y * 8 + x] = (descale(t[x], CONST_BITS + PASS1_BITS + 3) + 128).clamp(0, 255) as u8; }
    }
}

// A Huffman table: the largest code of each length and where its symbols start.
#[derive(Clone)]
struct Table { max: [i32; 18], offset: [i32; 17], symbols: Vec<u8> }

impl Table {
    fn new(counts: &[u8], symbols: &[u8]) -> Self {
        let (mut max, mut offset) = ([-1i32; 18], [0i32; 17]);
        let (mut code, mut k) = (0i32, 0i32);
        for len in 1..=16 {
            let n = counts[len - 1] as i32;
            offset[len] = k - code;
            if n > 0 { code += n; k += n; max[len] = code - 1; }
            code <<= 1;
        }
        max[17] = i32::MAX;
        Self { max, offset, symbols: symbols.to_vec() }
    }
}

// The entropy-coded data: bits, most significant first, with 0xFF 0x00 a 0xFF byte; a marker or the data's end stops
// it, and zero bytes stand for what follows (`padded` of them), so a take past the data is seen.
struct Bits<'a> { data: &'a [u8], at: usize, bit: u32, held: u32, marker: bool, padded: u32 }

impl Bits<'_> {
    fn fill(&mut self) {
        while self.held <= 24 {
            let mut byte = None;
            if !self.marker && self.at < self.data.len() {
                let b = self.data[self.at];
                if b == 0xFF {
                    let next = self.data.get(self.at + 1).copied().unwrap_or(0xD9);
                    if next == 0x00 { self.at += 2; byte = Some(0xFF); } else { self.marker = true; }
                } else { self.at += 1; byte = Some(b as u32); }
            }
            let byte = byte.unwrap_or_else(|| { self.padded += 1; 0 });
            self.bit |= byte << (24 - self.held);
            self.held += 8;
        }
    }
    fn take(&mut self, n: u32) -> u32 {
        if n == 0 { return 0; }
        self.fill();
        let value = self.bit >> (32 - n);
        self.bit <<= n; self.held -= n;
        value
    }
    fn symbol(&mut self, table: &Table) -> Result<u8, Error> {
        let mut code = 0i32;
        for len in 1..=16 {
            code = code << 1 | self.take(1) as i32;
            if code <= table.max[len] { return table.symbols.get((code + table.offset[len]) as usize).copied().ok_or("a bad Huffman code"); }
        }
        Err("a Huffman code longer than 16 bits")
    }
    // A value of `n` bits (at most 16) as JPEG signs it.
    fn value(&mut self, n: u32) -> i32 {
        if n == 0 { return 0; }
        let v = self.take(n) as i32;
        if v < 1 << (n - 1) { v - (1 << n) + 1 } else { v }
    }
    // Whether bits past the data were taken: the padding the data's end or a marker left (they come last).
    fn overrun(&self) -> bool { self.padded * 8 > self.held }
    // At a restart: the bits left go, the RSTn marker is passed.
    fn restart(&mut self) {
        (self.bit, self.held, self.marker, self.padded) = (0, 0, false, 0);
        while self.at + 1 < self.data.len() && !(self.data[self.at] == 0xFF && (0xD0..=0xD7).contains(&self.data[self.at + 1])) { self.at += 1; }
        self.at = (self.at + 2).min(self.data.len());
    }
}

struct Component { id: u8, h: usize, v: usize, quant: usize, dc: usize, ac: usize, pred: i32 }

/// The picture's width and height, from its frame header.
pub fn size(data: &[u8]) -> Option<(usize, usize)> {
    let mut at = 2;
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 { return None; }
    while at + 4 <= data.len() {
        if data[at] != 0xFF { return None; }
        let marker = data[at + 1];
        let len = u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize;
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) && at + 9 <= data.len() {
            return Some((u16::from_be_bytes([data[at + 7], data[at + 8]]) as usize, u16::from_be_bytes([data[at + 5], data[at + 6]]) as usize));
        }
        at += 2 + len;
    }
    None
}

/// Decodes `data`, handing each row (its number, its pixels) to `row`. Refuses pictures wider or taller than `limit`.
pub fn decode(data: &[u8], limit: usize, row: &mut dyn FnMut(usize, &[u32])) -> Result<(usize, usize), Error> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 { return Err("not a JPEG"); }
    let mut quant = [[0i32; 64]; 4];
    let (mut dc_tables, mut ac_tables): (Vec<Option<Table>>, Vec<Option<Table>>) = (vec![None; 4], vec![None; 4]);
    let mut components: Vec<Component> = Vec::new();
    let (mut width, mut height, mut interval) = (0usize, 0usize, 0usize);
    let mut at = 2;
    loop {
        // Fill bytes between markers are allowed.
        while at < data.len() && data[at] == 0xFF && data.get(at + 1) == Some(&0xFF) { at += 1; }
        if at + 4 > data.len() || data[at] != 0xFF { return Err(CUT_SHORT); }
        let marker = data[at + 1];
        let len = u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize;
        let body = data.get(at + 4..at + 2 + len).ok_or("a JPEG segment cut short")?;
        match marker {
            0xDB => {
                let mut p = 0;
                while p < body.len() {
                    let (precision, id) = (body[p] >> 4, (body[p] & 3) as usize);
                    p += 1;
                    for (i, &z) in ZIGZAG.iter().enumerate() {
                        quant[id][z] = if precision == 0 { *body.get(p + i).ok_or("a short table")? as i32 } else { u16::from_be_bytes([*body.get(p + 2 * i).ok_or("a short table")?, *body.get(p + 2 * i + 1).ok_or("a short table")?]) as i32 };
                    }
                    p += if precision == 0 { 64 } else { 128 };
                }
            }
            0xC4 => {
                let mut p = 0;
                while p + 17 <= body.len() {
                    let (class, id) = (body[p] >> 4, (body[p] & 3) as usize);
                    let counts = &body[p + 1..p + 17];
                    let n: usize = counts.iter().map(|&c| c as usize).sum();
                    let symbols = body.get(p + 17..p + 17 + n).ok_or("a short Huffman table")?;
                    let table = Some(Table::new(counts, symbols));
                    if class == 0 { dc_tables[id] = table; } else { ac_tables[id] = table; }
                    p += 17 + n;
                }
            }
            0xC0 | 0xC1 => {
                if !components.is_empty() { return Err("a JPEG of two frame headers"); }
                if body.len() < 6 || body[0] != 8 { return Err("a JPEG that is not 8 bits a sample"); }
                height = u16::from_be_bytes([body[1], body[2]]) as usize;
                width = u16::from_be_bytes([body[3], body[4]]) as usize;
                let count = body[5] as usize;
                if count != 1 && count != 3 { return Err("a JPEG that is neither greyscale nor YCbCr"); }
                for i in 0..count {
                    let c = body.get(6 + i * 3..9 + i * 3).ok_or("a short frame header")?;
                    let (h, v) = ((c[1] >> 4) as usize, (c[1] & 15) as usize);
                    if !(1..=4).contains(&h) || !(1..=4).contains(&v) { return Err("bad sampling factors"); }
                    components.push(Component { id: c[0], h, v, quant: (c[2] & 3) as usize, dc: 0, ac: 0, pred: 0 });
                }
            }
            0xC2 | 0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return Err("a progressive or lossless JPEG"),
            0xDD => interval = u16::from_be_bytes([*body.first().ok_or("a short restart interval")?, *body.get(1).ok_or("a short restart interval")?]) as usize,
            0xDA => {
                if components.is_empty() { return Err("a scan before the frame header"); }
                if width == 0 || height == 0 || width > limit || height > limit { return Err("too large a JPEG"); }
                let count = *body.first().ok_or("a short scan header")? as usize;
                if count != components.len() { return Err("a JPEG of several scans"); }
                if body.len() < 1 + count * 2 { return Err("a short scan header"); }
                for i in 0..count {
                    let (id, tables) = (body[1 + i * 2], body[2 + i * 2]);
                    let c = components.iter_mut().find(|c| c.id == id).ok_or("a scan of an unknown component")?;
                    (c.dc, c.ac) = ((tables >> 4) as usize & 3, (tables & 15) as usize & 3);
                }
                // A frame without tables takes the standard ones: 0 for luma, 1 for chroma (Annex K.3).
                for (n, (dc, ac)) in [(DC_LUMA, AC_LUMA), (DC_CHROMA, AC_CHROMA)].into_iter().enumerate() {
                    dc_tables[n].get_or_insert_with(|| Table::new(&dc.0, dc.1));
                    ac_tables[n].get_or_insert_with(|| Table::new(&ac.0, ac.1));
                }
                return scan(&data[at + 2 + len..], &mut components, &quant, &dc_tables, &ac_tables, (width, height), interval, row).map(|()| (width, height));
            }
            0xD9 => return Err("a JPEG without a scan"),
            _ => {}
        }
        at += 2 + len;
    }
}

// The scan: rows of blocks decoded, turned to pixels and handed on.
#[allow(clippy::too_many_arguments)]
fn scan(data: &[u8], components: &mut [Component], quant: &[[i32; 64]; 4], dc: &[Option<Table>], ac: &[Option<Table>], (width, height): (usize, usize), interval: usize, row: &mut dyn FnMut(usize, &[u32])) -> Result<(), Error> {
    let (hmax, vmax) = (components.iter().map(|c| c.h).max().unwrap_or(1), components.iter().map(|c| c.v).max().unwrap_or(1));
    let single = components.len() == 1;
    // One component alone is coded in blocks of its own, not in MCUs of the factors.
    let (mcu_w, mcu_h) = if single { (8, 8) } else { (hmax * 8, vmax * 8) };
    let (mcus_x, mcus_y) = (width.div_ceil(mcu_w), height.div_ceil(mcu_h));
    let blocks = |c: &Component| if single { (1, 1) } else { (c.h, c.v) };
    let mut planes: Vec<Vec<u8>> = components.iter().map(|c| { let (h, v) = blocks(c); vec![0u8; mcus_x * h * 8 * v * 8] }).collect();
    // Each component's sample for a pixel's column, and its plane's width: computed once, not a division a pixel.
    let columns: Vec<Vec<usize>> = components.iter().map(|c| { let h = blocks(c).0; (0..width).map(|x| if single { x } else { x * h / hmax }).collect() }).collect();
    let widths: Vec<usize> = components.iter().map(|c| mcus_x * blocks(c).0 * 8).collect();
    let mut bits = Bits { data, at: 0, bit: 0, held: 0, marker: false, padded: 0 };
    let mut samples = [0u8; 64];
    let mut pixels = vec![0u32; width];
    let mut done = 0usize;
    for my in 0..mcus_y {
        for mx in 0..mcus_x {
            if interval > 0 && done > 0 && done % interval == 0 {
                if bits.overrun() { return Err(CUT_SHORT); }
                bits.restart();
                for c in components.iter_mut() { c.pred = 0; }
            }
            done += 1;
            for (ci, c) in components.iter_mut().enumerate() {
                let (h, v) = blocks(c);
                let (dct, act) = (dc[c.dc].as_ref().ok_or("a missing DC table")?, ac[c.ac].as_ref().ok_or("a missing AC table")?);
                let q = &quant[c.quant];
                let plane_w = widths[ci];
                for by in 0..v {
                    for bx in 0..h {
                        let mut coefficients = [0i32; 64];
                        // Baseline 8-bit sizes: 11 bits a DC difference, 10 an AC value; values as libjpeg keeps them, 16 bits.
                        let s = bits.symbol(dct)? as u32;
                        if s > 11 { return Err("a DC difference of more than 11 bits"); }
                        c.pred = c.pred.saturating_add(bits.value(s));
                        coefficients[0] = c.pred.saturating_mul(q[0]).clamp(-32768, 32767);
                        let (mut k, mut flat) = (1, true);
                        while k < 64 {
                            let rs = bits.symbol(act)?;
                            let (run, size) = ((rs >> 4) as usize, (rs & 15) as u32);
                            if size == 0 { if run == 15 { k += 16; continue; } break; }
                            if size > 10 { return Err("an AC value of more than 10 bits"); }
                            k += run;
                            if k > 63 { return Err("a block of too many coefficients"); }
                            coefficients[ZIGZAG[k]] = (bits.value(size) * q[ZIGZAG[k]]).clamp(-32768, 32767);
                            flat = false;
                            k += 1;
                        }
                        idct(&coefficients, flat, &mut samples);
                        let (x0, y0) = ((mx * h + bx) * 8, by * 8);
                        for y in 0..8 { planes[ci][(y0 + y) * plane_w + x0..(y0 + y) * plane_w + x0 + 8].copy_from_slice(&samples[y * 8..y * 8 + 8]); }
                    }
                }
            }
        }
        // A row of blocks that took bits past the data: the picture was cut short, and is not shown half made.
        if bits.overrun() { return Err(CUT_SHORT); }
        // The row of blocks as pixels: each component sampled at its own resolution.
        for y in 0..mcu_h {
            let py = my * mcu_h + y;
            if py >= height { break; }
            let line = |ci: usize| { let v = blocks(&components[ci]).1; &planes[ci][(if single { y } else { y * v / vmax }) * widths[ci]..] };
            if single {
                let (l, cols) = (line(0), &columns[0]);
                for (x, p) in pixels.iter_mut().enumerate() { let g = l[cols[x]] as u32; *p = g << 16 | g << 8 | g; }
            } else {
                let (ly, lb, lr) = (line(0), line(1), line(2));
                for (x, p) in pixels.iter_mut().enumerate() {
                    let (l, cb, cr) = ((ly[columns[0][x]] as i32) << 16, lb[columns[1][x]] as i32 - 128, lr[columns[2][x]] as i32 - 128);
                    let r = (l + 91_881 * cr + 32_768) >> 16;
                    let g = (l - 22_554 * cb - 46_802 * cr + 32_768) >> 16;
                    let b = (l + 116_130 * cb + 32_768) >> 16;
                    *p = (r.clamp(0, 255) as u32) << 16 | (g.clamp(0, 255) as u32) << 8 | b.clamp(0, 255) as u32;
                }
            }
            row(py, &pixels);
        }
    }
    Ok(())
}
