//! Baseline JPEG pictures (000-APP-0050's backgrounds) decoded a row of blocks at a time: 8-bit greyscale or YCbCr,
//! any sampling factors, Huffman coding with restart markers. Rows go to a sink as `0x00RRGGBB` pixels as each row of
//! blocks is done, so a large photo is never held whole. Progressive and 12-bit pictures are refused. No system
//! calls: host-tested in tests/image_host.rs.
use crate::jpeg::ZIGZAG;
use alloc::vec;
use alloc::vec::Vec;

pub type Error = &'static str;

// cos(kπ/16) × 4096, k = 0..8.
const COS: [i32; 9] = [4096, 4017, 3784, 3406, 2896, 2276, 1567, 799, 0];

// T[u][x] = c(u) · cos((2x + 1)uπ/16) × 4096, c(0) = 1/√2.
const IDCT: [[i32; 8]; 8] = {
    let mut t = [[0i32; 8]; 8];
    let mut u = 0;
    while u < 8 {
        let mut x = 0;
        while x < 8 {
            let m = ((2 * x + 1) * u) % 32;
            let c = if m <= 8 { COS[m] } else if m <= 16 { -COS[16 - m] } else if m <= 24 { -COS[m - 16] } else { COS[32 - m] };
            t[u][x] = if u == 0 { c * 2896 / 4096 } else { c };
            x += 1;
        }
        u += 1;
    }
    t
};

// 8 × 8 coefficients (natural order, dequantized) into samples, 0..=255.
fn idct(block: &[i32; 64], out: &mut [u8; 64]) {
    let mut tmp = [0i32; 64];
    for y in 0..8 {
        let row = &block[y * 8..y * 8 + 8];
        if row[1..].iter().all(|&c| c == 0) {
            let dc = row[0] * IDCT[0][0] >> 8;
            for x in 0..8 { tmp[y * 8 + x] = dc; }
            continue;
        }
        for x in 0..8 { tmp[y * 8 + x] = (0..8).map(|u| row[u] * IDCT[u][x]).sum::<i32>() >> 8; }
    }
    for x in 0..8 {
        for y in 0..8 {
            let sum: i64 = (0..8).map(|v| tmp[v * 8 + x] as i64 * IDCT[v][y] as i64).sum();
            out[y * 8 + x] = ((sum + (1 << 17)) >> 18).wrapping_add(128).clamp(0, 255) as u8;
        }
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

// The entropy-coded data: bits, most significant first, with 0xFF 0x00 a 0xFF byte; a marker stops it.
struct Bits<'a> { data: &'a [u8], at: usize, bit: u32, held: u32, marker: bool }

impl Bits<'_> {
    fn fill(&mut self) {
        while self.held <= 24 {
            let mut byte = 0u32;
            if !self.marker && self.at < self.data.len() {
                let b = self.data[self.at];
                if b == 0xFF {
                    let next = self.data.get(self.at + 1).copied().unwrap_or(0xD9);
                    if next == 0x00 { self.at += 2; byte = 0xFF; } else { self.marker = true; }
                } else { self.at += 1; byte = b as u32; }
            }
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
    // A value of `n` bits as JPEG signs it.
    fn value(&mut self, n: u32) -> i32 {
        if n == 0 { return 0; }
        let v = self.take(n) as i32;
        if v < 1 << (n - 1) { v - (1 << n) + 1 } else { v }
    }
    // At a restart: the bits left go, the RSTn marker is passed.
    fn restart(&mut self) {
        self.bit = 0; self.held = 0; self.marker = false;
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
        if at + 4 > data.len() || data[at] != 0xFF { return Err("a JPEG cut short"); }
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
    let mut bits = Bits { data, at: 0, bit: 0, held: 0, marker: false };
    let mut samples = [0u8; 64];
    let mut pixels = vec![0u32; width];
    let mut done = 0usize;
    for my in 0..mcus_y {
        for mx in 0..mcus_x {
            if interval > 0 && done > 0 && done % interval == 0 { bits.restart(); for c in components.iter_mut() { c.pred = 0; } }
            done += 1;
            for (ci, c) in components.iter_mut().enumerate() {
                let (h, v) = blocks(c);
                let (dct, act) = (dc[c.dc].as_ref().ok_or("a missing DC table")?, ac[c.ac].as_ref().ok_or("a missing AC table")?);
                let q = &quant[c.quant];
                let plane_w = mcus_x * h * 8;
                for by in 0..v {
                    for bx in 0..h {
                        let mut coefficients = [0i32; 64];
                        let s = bits.symbol(dct)? as u32;
                        c.pred += bits.value(s);
                        coefficients[0] = c.pred * q[0];
                        let mut k = 1;
                        while k < 64 {
                            let rs = bits.symbol(act)?;
                            let (run, size) = ((rs >> 4) as usize, (rs & 15) as u32);
                            if size == 0 { if run == 15 { k += 16; continue; } break; }
                            k += run;
                            if k > 63 { return Err("a block of too many coefficients"); }
                            coefficients[ZIGZAG[k]] = bits.value(size) * q[ZIGZAG[k]];
                            k += 1;
                        }
                        idct(&coefficients, &mut samples);
                        let (x0, y0) = ((mx * h + bx) * 8, by * 8);
                        for y in 0..8 { planes[ci][(y0 + y) * plane_w + x0..(y0 + y) * plane_w + x0 + 8].copy_from_slice(&samples[y * 8..y * 8 + 8]); }
                    }
                }
            }
        }
        // The row of blocks as pixels: each component sampled at its own resolution.
        for y in 0..mcu_h {
            let py = my * mcu_h + y;
            if py >= height { break; }
            for (x, p) in pixels.iter_mut().enumerate() {
                let at = |ci: usize| { let (h, v) = blocks(&components[ci]); let w = mcus_x * h * 8; planes[ci][(y * v / if single { 1 } else { vmax }) * w + x * h / if single { 1 } else { hmax }] as i32 };
                *p = if single { let l = at(0) as u32; l << 16 | l << 8 | l } else {
                    let (l, cb, cr) = (at(0) << 16, at(1) - 128, at(2) - 128);
                    let r = (l + 91_881 * cr + 32_768) >> 16;
                    let g = (l - 22_554 * cb - 46_802 * cr + 32_768) >> 16;
                    let b = (l + 116_130 * cb + 32_768) >> 16;
                    (r.clamp(0, 255) as u32) << 16 | (g.clamp(0, 255) as u32) << 8 | b.clamp(0, 255) as u32
                };
            }
            row(py, &pixels);
        }
    }
    Ok(())
}
