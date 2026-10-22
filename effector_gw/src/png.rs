//! The screen as a PNG for `open-screen` (750-EFF-0007): the copy shrunk to fit a bound by averaging boxes of
//! pixels, each row filtered by the PNG filter whose bytes sum smallest, and one zlib stream of a fixed-Huffman deflate
//! block with LZ77 matches over a 32 KiB window. No system calls: host-tested in tests/effector_png_host.rs, decoded
//! there by `mind::png`.
use alloc::vec;
use alloc::vec::Vec;

const WINDOW: usize = 32768;
const MATCH_MIN: usize = 3;
const MATCH_MAX: usize = 258;
const CHAIN: usize = 48; // candidates tried per position
const HASH_BITS: u32 = 15;

/// The size a `width` x `height` screen takes within `max_width` x `max_height`, its proportions kept; never larger.
pub fn fit(width: usize, height: usize, max_width: usize, max_height: usize) -> (usize, usize) {
    if width <= max_width && height <= max_height { return (width, height); }
    // The tighter of the two ratios, compared without division: max_width / width against max_height / height.
    if max_width * height <= max_height * width {
        (max_width, (height * max_width / width).max(1))
    } else {
        ((width * max_height / height).max(1), max_height)
    }
}

/// Row `y` of the picture `pixels` (`0x00RRGGBB`, `stride` pixels a row, `width` x `height`) shrunk to `out.len()`
/// x `to_height`: each pixel the mean of the box of source pixels it covers.
pub fn shrink_row(pixels: &[u32], stride: usize, width: usize, height: usize, to_height: usize, y: usize, out: &mut [u32]) {
    let to_width = out.len();
    let (y0, y1) = (y * height / to_height, ((y + 1) * height / to_height).max(y * height / to_height + 1).min(height));
    for (x, pixel) in out.iter_mut().enumerate() {
        let (x0, x1) = (x * width / to_width, ((x + 1) * width / to_width).max(x * width / to_width + 1).min(width));
        let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
        for sy in y0..y1 {
            for &p in &pixels[sy * stride + x0..sy * stride + x1] { r += p >> 16 & 255; g += p >> 8 & 255; b += p & 255; }
        }
        let n = ((y1 - y0) * (x1 - x0)) as u32;
        *pixel = (r + n / 2) / n << 16 | (g + n / 2) / n << 8 | (b + n / 2) / n;
    }
}

/// An 8-bit RGB PNG of `width` x `height` pixels; `row(y, out)` gives row `y` as `0x00RRGGBB`.
pub fn encode(width: usize, height: usize, row: &mut dyn FnMut(usize, &mut [u32])) -> Vec<u8> {
    let line = 3 * width;
    let mut raw = Vec::with_capacity(height * (line + 1));
    let (mut pixels, mut previous, mut current) = (vec![0u32; width], vec![0u8; line], vec![0u8; line]);
    let mut candidate = vec![0u8; line];
    let mut best = vec![0u8; line];
    for y in 0..height {
        row(y, &mut pixels);
        for (x, p) in pixels.iter().enumerate() {
            current[3 * x] = (p >> 16) as u8; current[3 * x + 1] = (p >> 8) as u8; current[3 * x + 2] = *p as u8;
        }
        let mut chosen = (u64::MAX, 0u8);
        for kind in 0..5u8 {
            filter(kind, &current, &previous, &mut candidate);
            let cost: u64 = candidate.iter().map(|&b| (b as i8).unsigned_abs() as u64).sum();
            if cost < chosen.0 { chosen = (cost, kind); best.copy_from_slice(&candidate); }
        }
        raw.push(chosen.1);
        raw.extend_from_slice(&best);
        core::mem::swap(&mut previous, &mut current);
    }
    let mut png = Vec::with_capacity(raw.len() / 8 + 64);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&(width as u32).to_be_bytes());
    header[4..8].copy_from_slice(&(height as u32).to_be_bytes());
    header[8] = 8; // bits a sample
    header[9] = 2; // RGB
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib(&raw));
    chunk(&mut png, b"IEND", &[]);
    png
}

// The PNG filters, RGB at 3 bytes a pixel: 0 none, 1 sub, 2 up, 3 average, 4 Paeth.
fn filter(kind: u8, row: &[u8], above: &[u8], out: &mut [u8]) {
    for i in 0..row.len() {
        let (a, b, c) = (if i >= 3 { row[i - 3] } else { 0 }, above[i], if i >= 3 { above[i - 3] } else { 0 });
        out[i] = row[i].wrapping_sub(match kind {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((a as u16 + b as u16) / 2) as u8,
            _ => paeth(a, b, c),
        });
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let crc = crc32(&png[start..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 { crc = if crc & 1 != 0 { crc >> 1 ^ 0xEDB8_8320 } else { crc >> 1 }; }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for piece in data.chunks(5552) {
        for &x in piece { a += x as u32; b += a; }
        a %= 65521; b %= 65521;
    }
    b << 16 | a
}

/// Bits out least significant first, as deflate packs them.
struct Bits { out: Vec<u8>, bits: u32, count: u32 }
impl Bits {
    fn put(&mut self, value: u32, n: u32) {
        self.bits |= value << self.count;
        self.count += n;
        while self.count >= 8 { self.out.push(self.bits as u8); self.bits >>= 8; self.count -= 8; }
    }
    /// A Huffman code, whose bits deflate sends most significant first.
    fn code(&mut self, code: u32, n: u32) { self.put(code.reverse_bits() >> (32 - n), n); }
    fn literal(&mut self, symbol: u32) {
        match symbol {
            0..=143 => self.code(0x30 + symbol, 8),
            144..=255 => self.code(0x190 + symbol - 144, 9),
            256..=279 => self.code(symbol - 256, 7),
            _ => self.code(0xC0 + symbol - 280, 8),
        }
    }
}

const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DISTANCE_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DISTANCE_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn hash(d: &[u8], i: usize) -> usize {
    ((d[i] as u32) << 16 | (d[i + 1] as u32) << 8 | d[i + 2] as u32).wrapping_mul(2_654_435_761) as usize >> (32 - HASH_BITS)
}

/// `data` as a zlib stream: one final fixed-Huffman block.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits { out: Vec::with_capacity(data.len() / 8 + 16), bits: 0, count: 0 };
    bits.out.extend_from_slice(&[0x78, 0x01]);
    bits.put(1, 1); // the last block
    bits.put(1, 2); // fixed Huffman codes
    let (mut head, mut previous) = (vec![u32::MAX; 1 << HASH_BITS], vec![u32::MAX; WINDOW]);
    let insert = |i: usize, head: &mut Vec<u32>, previous: &mut Vec<u32>| {
        if i + MATCH_MIN <= data.len() { let h = hash(data, i); previous[i % WINDOW] = head[h]; head[h] = i as u32; }
    };
    let mut i = 0;
    while i < data.len() {
        let (mut length, mut distance) = (0, 0);
        if i + MATCH_MIN <= data.len() {
            let longest = (data.len() - i).min(MATCH_MAX);
            let mut candidate = head[hash(data, i)];
            for _ in 0..CHAIN {
                if candidate == u32::MAX { break; }
                let c = candidate as usize;
                if c >= i || i - c >= WINDOW { break; }
                if data[c + length.min(longest - 1)] == data[i + length.min(longest - 1)] {
                    let n = data[c..c + longest].iter().zip(&data[i..i + longest]).take_while(|(a, b)| a == b).count();
                    if n > length { length = n; distance = i - c; if n == longest { break; } }
                }
                candidate = previous[c % WINDOW];
            }
        }
        if length >= MATCH_MIN {
            let code = LENGTH_BASE.iter().rposition(|&b| b as usize <= length).unwrap();
            bits.literal(257 + code as u32);
            bits.put((length - LENGTH_BASE[code] as usize) as u32, LENGTH_EXTRA[code] as u32);
            let code = DISTANCE_BASE.iter().rposition(|&b| b as usize <= distance).unwrap();
            bits.code(code as u32, 5);
            bits.put((distance - DISTANCE_BASE[code] as usize) as u32, DISTANCE_EXTRA[code] as u32);
            for at in i..i + length { insert(at, &mut head, &mut previous); }
            i += length;
        } else {
            bits.literal(data[i] as u32);
            insert(i, &mut head, &mut previous);
            i += 1;
        }
    }
    bits.literal(256);
    if bits.count > 0 { bits.out.push(bits.bits as u8); }
    let mut out = bits.out;
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}
