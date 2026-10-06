//! A baseline JPEG encoder (issue 093) for `record`'s Motion JPEG: 8-bit YCbCr with 4:2:0 chroma, the quantization
//! tables of the JPEG standard (Annex K) scaled by a quality as libjpeg scales them, libjpeg's accurate integer DCT
//! (integers: fast under emulation too), and the standard Huffman tables (written into every image, so any decoder
//! reads it). A restart marker ends every row of 16×16 blocks: each row's data stands alone, so a frame like the one
//! before it encodes only the rows that changed and takes the others from `Rows`. Pixels are `0x00RRGGBB`, as the
//! screen and its captures hold them. No system calls: tests/jpeg_host.rs decodes what it writes.
use alloc::vec::Vec;

/// The order in which a block's coefficients are written: natural index of each zigzag position.
pub const ZIGZAG: [usize; 64] = [0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
                                 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63];

const LUMA: [u8; 64] = [16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80, 62,
                        18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99];
const CHROMA: [u8; 64] = [17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99, 99,
                          99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99];

/// The standard Huffman tables (Annex K.3): code counts for the lengths 1-16, then the symbols.
pub const DC_LUMA: ([u8; 16], &[u8]) = ([0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
pub const DC_CHROMA: ([u8; 16], &[u8]) = ([0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
pub const AC_LUMA: ([u8; 16], &[u8]) = ([0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D], &[
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
    0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
    0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA]);
pub const AC_CHROMA: ([u8; 16], &[u8]) = ([0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77], &[
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91,
    0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0, 0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
    0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4,
    0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA]);


/// A Huffman code per symbol: (code, length in bits).
type Codes = [(u16, u8); 256];

fn codes(table: &([u8; 16], &[u8])) -> Codes {
    let mut codes = [(0u16, 0u8); 256];
    let (mut code, mut k) = (0u16, 0usize);
    for (length, &count) in table.0.iter().enumerate() {
        for _ in 0..count { codes[table.1[k] as usize] = (code, length as u8 + 1); code += 1; k += 1; }
        code <<= 1;
    }
    codes
}

/// The quantization table for `quality` (1-100), scaled as libjpeg scales the standard ones; natural order.
pub fn quantization(base: &[u8; 64], quality: u8) -> [u8; 64] {
    let quality = quality.clamp(1, 100) as u32;
    let scale = if quality < 50 { 5000 / quality } else { 200 - 2 * quality };
    let mut table = [0u8; 64];
    for (t, &b) in table.iter_mut().zip(base) { *t = ((b as u32 * scale + 50) / 100).clamp(1, 255) as u8; }
    table
}

/// An encoder for one quality: its tables made once, used for every frame.
pub struct Encoder { quant: [[u8; 64]; 2], dc: [Codes; 2], ac: [Codes; 2] }

/// The coded rows of blocks of the last frame, kept for the next one (`Encoder::encode_again`).
#[derive(Default)]
pub struct Rows { coded: Vec<Vec<u8>> }

// Bits written most significant first, 0xFF followed by 0x00 in the data.
struct Bits<'a> { out: &'a mut Vec<u8>, acc: u32, count: u32 }

impl Bits<'_> {
    fn put(&mut self, code: u32, length: u32) {
        self.acc = (self.acc << length) | (code & ((1 << length) - 1));
        self.count += length;
        while self.count >= 8 {
            let byte = (self.acc >> (self.count - 8)) as u8;
            self.out.push(byte);
            if byte == 0xFF { self.out.push(0); }
            self.count -= 8;
        }
        self.acc &= (1 << self.count) - 1;
    }
    // The last byte filled with ones.
    fn finish(&mut self) { if self.count > 0 { self.put(0x7F, 8 - self.count); } }
}

// The number of bits of |value| (its JPEG category).
fn category(value: i32) -> u32 { 32 - value.unsigned_abs().leading_zeros() }

// libjpeg's accurate integer DCT (jfdctint): rows, then columns, in place; the results are 8 times the DCT's.
const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;
fn descale(x: i32, n: i32) -> i32 { (x + (1 << (n - 1))) >> n }

fn fdct(data: &mut [i32; 64]) {
    for pass in 0..2 {
        for i in 0..8 {
            let at = |k: usize| if pass == 0 { i * 8 + k } else { k * 8 + i };
            let d: [i32; 8] = core::array::from_fn(|k| data[at(k)]);
            let (t0, t7, t1, t6) = (d[0] + d[7], d[0] - d[7], d[1] + d[6], d[1] - d[6]);
            let (t2, t5, t3, t4) = (d[2] + d[5], d[2] - d[5], d[3] + d[4], d[3] - d[4]);
            let (t10, t13, t11, t12) = (t0 + t3, t0 - t3, t1 + t2, t1 - t2);
            // The first pass keeps PASS1_BITS more bits; the second takes them out.
            let (even, odd) = if pass == 0 { (CONST_BITS - PASS1_BITS, CONST_BITS - PASS1_BITS) } else { (PASS1_BITS, CONST_BITS + PASS1_BITS) };
            if pass == 0 { data[at(0)] = (t10 + t11) << PASS1_BITS; data[at(4)] = (t10 - t11) << PASS1_BITS; }
            else { data[at(0)] = descale(t10 + t11, even); data[at(4)] = descale(t10 - t11, even); }
            let z1 = (t12 + t13) * 4433;
            data[at(2)] = descale(z1 + t13 * 6270, odd);
            data[at(6)] = descale(z1 - t12 * 15137, odd);
            let (z1, z2, z3, z4) = (t4 + t7, t5 + t6, t4 + t6, t5 + t7);
            let z5 = (z3 + z4) * 9633;
            let (t4, t5, t6, t7) = (t4 * 2446, t5 * 16819, t6 * 25172, t7 * 12299);
            let (z1, z2, z3, z4) = (z1 * -7373, z2 * -20995, z3 * -16069 + z5, z4 * -3196 + z5);
            data[at(7)] = descale(t4 + z1 + z3, odd);
            data[at(5)] = descale(t5 + z2 + z4, odd);
            data[at(3)] = descale(t6 + z2 + z3, odd);
            data[at(1)] = descale(t7 + z1 + z4, odd);
        }
    }
}

// A DCT result over its quantizer (times 8, as the DCT leaves it), rounded to the nearest.
fn quantize(value: i32, q: u8) -> i32 {
    let d = q as i32 * 8;
    if value >= 0 { (value + d / 2) / d } else { -((-value + d / 2) / d) }
}

impl Encoder {
    pub fn new(quality: u8) -> Self {
        let quant = [quantization(&LUMA, quality), quantization(&CHROMA, quality)];
        Self { quant, dc: [codes(&DC_LUMA), codes(&DC_CHROMA)], ac: [codes(&AC_LUMA), codes(&AC_CHROMA)] }
    }

    fn header(&self, out: &mut Vec<u8>, width: usize, height: usize) {
        out.extend_from_slice(&[0xFF, 0xD8]); // SOI
        out.extend_from_slice(&[0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0]); // JFIF 1.1, no density
        for (t, table) in self.quant.iter().enumerate() {
            out.extend_from_slice(&[0xFF, 0xDB, 0, 67, t as u8]);
            out.extend(ZIGZAG.iter().map(|&n| table[n]));
        }
        let (w, h) = ((width as u16).to_be_bytes(), (height as u16).to_be_bytes());
        // Y sampled 2×2, Cb and Cr once per 16×16.
        out.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8, h[0], h[1], w[0], w[1], 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        // A restart after every row of blocks.
        out.extend_from_slice(&[0xFF, 0xDD, 0, 4]);
        out.extend_from_slice(&(width.div_ceil(16) as u16).to_be_bytes());
        for (class_id, table) in [(0x00, &DC_LUMA), (0x10, &AC_LUMA), (0x01, &DC_CHROMA), (0x11, &AC_CHROMA)] {
            let length = (2 + 1 + 16 + table.1.len()) as u16;
            out.extend_from_slice(&[0xFF, 0xC4]);
            out.extend_from_slice(&length.to_be_bytes());
            out.push(class_id);
            out.extend_from_slice(&table.0);
            out.extend_from_slice(table.1);
        }
        out.extend_from_slice(&[0xFF, 0xDA, 0, 12, 3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]); // SOS
    }

    // One block of level-shifted samples: DCT, quantization, then its Huffman codes; returns its DC.
    fn block(&self, bits: &mut Bits, data: &mut [i32; 64], table: usize, previous: i32) -> i32 {
        fdct(data);
        let q: [i32; 64] = core::array::from_fn(|k| quantize(data[ZIGZAG[k]], self.quant[table][ZIGZAG[k]]));
        let diff = q[0] - previous;
        let size = category(diff);
        let (code, length) = self.dc[table][size as usize];
        bits.put(code as u32, length as u32);
        if size > 0 { bits.put(if diff < 0 { (diff - 1) as u32 } else { diff as u32 }, size); }
        let mut run = 0;
        for &value in &q[1..] {
            if value == 0 { run += 1; continue; }
            while run > 15 { let (code, length) = self.ac[table][0xF0]; bits.put(code as u32, length as u32); run -= 16; }
            let size = category(value);
            let (code, length) = self.ac[table][(run << 4 | size) as usize];
            bits.put(code as u32, length as u32);
            bits.put(if value < 0 { (value - 1) as u32 } else { value as u32 }, size);
            run = 0;
        }
        if run > 0 { let (code, length) = self.ac[table][0]; bits.put(code as u32, length as u32); }
        q[0]
    }

    // The coded data of the row of blocks from line `top`: byte-aligned, as a restart interval is.
    fn row(&self, pixels: &[u32], width: usize, height: usize, stride: usize, top: usize, out: &mut Vec<u8>) {
        let mut bits = Bits { out, acc: 0, count: 0 };
        let mut dc = [0i32; 3];
        // A pixel's Y, Cb and Cr (each 0-255), the nearest one inside the image for a block past its edge.
        let ycc = |x: usize, y: usize| -> [i32; 3] {
            let p = pixels[y.min(height - 1) * stride + x.min(width - 1)];
            let (r, g, b) = ((p >> 16 & 0xFF) as i32, (p >> 8 & 0xFF) as i32, (p & 0xFF) as i32);
            [(19595 * r + 38470 * g + 7471 * b + 32768) >> 16,
             ((-11059 * r - 21709 * g + 32768 * b + 32768) >> 16) + 128,
             ((32768 * r - 27439 * g - 5329 * b + 32768) >> 16) + 128]
        };
        for mx in (0..width).step_by(16) {
            let mut chroma = [[0i32; 64]; 2];
            for by in 0..2 {
                for bx in 0..2 {
                    let mut block = [0i32; 64];
                    for y in 0..8 {
                        for x in 0..8 {
                            let [l, cb, cr] = ycc(mx + bx * 8 + x, top + by * 8 + y);
                            block[y * 8 + x] = l - 128;
                            let c = ((by * 8 + y) / 2) * 8 + (bx * 8 + x) / 2;
                            chroma[0][c] += cb;
                            chroma[1][c] += cr;
                        }
                    }
                    dc[0] = self.block(&mut bits, &mut block, 0, dc[0]);
                }
            }
            for (k, sums) in chroma.iter().enumerate() {
                let mut block: [i32; 64] = core::array::from_fn(|i| ((sums[i] + 2) >> 2) - 128);
                dc[k + 1] = self.block(&mut bits, &mut block, 1, dc[k + 1]);
            }
        }
        bits.finish();
    }

    /// `width` × `height` pixels (`stride` pixels a row) as a JPEG, appended to `out`.
    pub fn encode(&self, pixels: &[u32], width: usize, height: usize, stride: usize, out: &mut Vec<u8>) {
        self.encode_again(pixels, None, width, height, stride, &mut Rows::default(), out);
    }

    /// As `encode`, for a frame that follows `previous` (the same size): a row of blocks whose pixels are as they were
    /// is taken from `rows` instead of being coded again; `rows` then holds this frame's. Returns the rows coded.
    pub fn encode_again(&self, pixels: &[u32], previous: Option<&[u32]>, width: usize, height: usize, stride: usize, rows: &mut Rows, out: &mut Vec<u8>) -> usize {
        self.header(out, width, height);
        let count = height.div_ceil(16);
        if rows.coded.len() != count { rows.coded = (0..count).map(|_| Vec::new()).collect(); }
        let mut coded = 0;
        for (r, data) in rows.coded.iter_mut().enumerate() {
            let lines = r * 16 * stride..(r * 16 + 16).min(height) * stride;
            let same = previous.is_some_and(|p| p[lines.clone()] == pixels[lines.clone()]) && !data.is_empty();
            if !same { data.clear(); self.row(pixels, width, height, stride, r * 16, data); coded += 1; }
            out.extend_from_slice(data);
            if r + 1 < count { out.extend_from_slice(&[0xFF, 0xD0 + (r % 8) as u8]); }
        }
        out.extend_from_slice(&[0xFF, 0xD9]); // EOI
        coded
    }
}
