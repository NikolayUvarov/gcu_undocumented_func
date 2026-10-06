//! Host tests of `record`'s formats (issue 093): the JPEG encoder (libmind/src/jpeg.rs), read back by a small baseline
//! decoder here — the markers and tables it writes, then the pixels within an error bound of the original — and the
//! AVI writer (libmind/src/avi.rs): the header's sizes and counts, the chunks and the index.
extern crate alloc;
#[path = "../libmind/src/jpeg.rs"]
mod jpeg;
#[path = "../libmind/src/avi.rs"]
mod avi;

use std::collections::HashMap;

// ---- A baseline decoder: what the encoder writes (8-bit, three components, Y 2×2 and one Cb and Cr per 16×16). ----

struct Reader<'a> { data: &'a [u8], at: usize, acc: u32, count: u32 }

impl Reader<'_> {
    fn bit(&mut self) -> u32 {
        if self.count == 0 {
            let byte = self.data[self.at];
            self.at += 1;
            if byte == 0xFF { assert_eq!(self.data[self.at], 0, "0xFF in the data is followed by 0x00"); self.at += 1; }
            self.acc = byte as u32;
            self.count = 8;
        }
        self.count -= 1;
        (self.acc >> self.count) & 1
    }
    fn bits(&mut self, n: u32) -> i32 {
        let mut v = 0;
        for _ in 0..n { v = v << 1 | self.bit(); }
        // Values with the top bit clear are negative (JPEG's "extend").
        if n > 0 && v < 1 << (n - 1) { v as i32 - (1 << n) + 1 } else { v as i32 }
    }
    fn symbol(&mut self, table: &HashMap<(u8, u16), u8>) -> u8 {
        let (mut code, mut length) = (0u16, 0u8);
        loop {
            code = code << 1 | self.bit() as u16;
            length += 1;
            if let Some(&s) = table.get(&(length, code)) { return s; }
            assert!(length < 16, "no such code");
        }
    }
}

// The inverse DCT, rows then columns, from a table of the cosines.
fn idct(coefficients: &[f32; 64]) -> [f32; 64] {
    let basis: [[f32; 8]; 8] = std::array::from_fn(|x| std::array::from_fn(|u| {
        let c = if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
        c * ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos() / 2.0
    }));
    let mut rows = [0f32; 64];
    for v in 0..8 { for x in 0..8 { rows[v * 8 + x] = (0..8).map(|u| basis[x][u] * coefficients[v * 8 + u]).sum(); } }
    let mut out = [0f32; 64];
    for y in 0..8 { for x in 0..8 { out[y * 8 + x] = (0..8).map(|v| basis[y][v] * rows[v * 8 + x]).sum(); } }
    out
}

/// The image's width, height and pixels (0x00RRGGBB); panics on what the encoder should not write.
fn decode(file: &[u8]) -> (usize, usize, Vec<u32>) {
    assert_eq!(&file[..2], &[0xFF, 0xD8], "SOI");
    assert_eq!(&file[file.len() - 2..], &[0xFF, 0xD9], "EOI");
    let (mut quant, mut huffman) = (HashMap::new(), HashMap::new());
    let (mut width, mut height, mut at, mut interval) = (0, 0, 2, 0);
    loop {
        assert_eq!(file[at], 0xFF);
        let marker = file[at + 1];
        let length = u16::from_be_bytes([file[at + 2], file[at + 3]]) as usize;
        let body = &file[at + 4..at + 2 + length];
        match marker {
            0xE0 => assert_eq!(&body[..5], b"JFIF\0"),
            0xDD => interval = u16::from_be_bytes([body[0], body[1]]) as usize,
            0xDB => { let table: [u8; 64] = body[1..65].try_into().unwrap(); quant.insert(body[0], table); }
            0xC0 => {
                assert_eq!(body[0], 8);
                height = u16::from_be_bytes([body[1], body[2]]) as usize;
                width = u16::from_be_bytes([body[3], body[4]]) as usize;
                assert_eq!(&body[5..], &[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1], "Y 2×2 with table 0, Cb and Cr 1×1 with table 1");
            }
            0xC4 => {
                let (counts, symbols) = (&body[1..17], &body[17..]);
                let mut table = HashMap::new();
                let (mut code, mut k) = (0u16, 0);
                for (length, &n) in counts.iter().enumerate() {
                    for _ in 0..n { table.insert((length as u8 + 1, code), symbols[k]); code += 1; k += 1; }
                    code <<= 1;
                }
                assert_eq!(k, symbols.len());
                huffman.insert(body[0], table);
            }
            0xDA => { at += 2 + length; break; }
            other => panic!("marker {:#X}", other),
        }
        at += 2 + length;
    }
    let mut reader = Reader { data: &file[..file.len() - 2], at, acc: 0, count: 0 };
    let mut planes = [vec![0f32; width.div_ceil(16) * 16 * height.div_ceil(16) * 16], vec![0f32; width.div_ceil(16) * 8 * height.div_ceil(16) * 8], vec![0f32; width.div_ceil(16) * 8 * height.div_ceil(16) * 8]];
    let mut dc = [0i32; 3];
    let wide = [width.div_ceil(16) * 16, width.div_ceil(16) * 8, width.div_ceil(16) * 8];
    assert_eq!(interval, width.div_ceil(16), "a restart interval is a row of blocks");
    for my in 0..height.div_ceil(16) {
        if my > 0 {
            // A restart: the rest of the byte is padding, then RSTn, and the DC predictions start again.
            reader.count = 0;
            assert_eq!(&reader.data[reader.at..reader.at + 2], &[0xFF, 0xD0 + ((my - 1) % 8) as u8], "RST{}", (my - 1) % 8);
            reader.at += 2;
            dc = [0; 3];
        }
        for mx in 0..width.div_ceil(16) {
            for (component, blocks) in [(0usize, 4usize), (1, 1), (2, 1)] {
                let table = if component == 0 { 0 } else { 1 };
                for b in 0..blocks {
                    let mut zz = [0i32; 64];
                    let size = reader.symbol(&huffman[&table]);
                    dc[component] += reader.bits(size as u32);
                    zz[0] = dc[component];
                    let mut k = 1;
                    while k < 64 {
                        let rs = reader.symbol(&huffman[&(0x10 | table)]);
                        if rs == 0 { break; }
                        k += (rs >> 4) as usize;
                        zz[k] = reader.bits((rs & 15) as u32);
                        k += 1;
                    }
                    let q = &quant[&table];
                    let mut coefficients = [0f32; 64];
                    for (i, &z) in jpeg::ZIGZAG.iter().enumerate() { coefficients[z] = (zz[i] * q[i] as i32) as f32; }
                    let block = idct(&coefficients);
                    let (ox, oy) = if component == 0 { (mx * 16 + (b % 2) * 8, my * 16 + (b / 2) * 8) } else { (mx * 8, my * 8) };
                    for y in 0..8 { for x in 0..8 { planes[component][(oy + y) * wide[component] + ox + x] = block[y * 8 + x] + 128.0; } }
                }
            }
        }
    }
    let clamp = |v: f32| v.round().clamp(0.0, 255.0) as u32;
    let pixels = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).map(|(x, y)| {
        let l = planes[0][y * wide[0] + x];
        let (cb, cr) = (planes[1][(y / 2) * wide[1] + x / 2] - 128.0, planes[2][(y / 2) * wide[2] + x / 2] - 128.0);
        clamp(l + 1.402 * cr) << 16 | clamp(l - 0.344136 * cb - 0.714136 * cr) << 8 | clamp(l + 1.772 * cb)
    }).collect();
    (width, height, pixels)
}

fn channels(p: u32) -> [i32; 3] { [(p >> 16 & 0xFF) as i32, (p >> 8 & 0xFF) as i32, (p & 0xFF) as i32] }

// The mean absolute error per channel and the peak signal-to-noise ratio of `got` against `want`.
fn error(want: &[u32], got: &[u32]) -> (f64, f64) {
    let (mut absolute, mut square) = (0f64, 0f64);
    for (&a, &b) in want.iter().zip(got) {
        for (x, y) in channels(a).iter().zip(channels(b)) { let d = (x - y) as f64; absolute += d.abs(); square += d * d; }
    }
    let n = (want.len() * 3) as f64;
    (absolute / n, 10.0 * (255.0f64 * 255.0 / (square / n)).log10())
}

// A test picture: smooth gradients, flat areas and a sharp edge, `width` × `height`, in a wider buffer.
fn picture(width: usize, height: usize, stride: usize) -> Vec<u32> {
    let mut pixels = vec![0xDEAD_BEEF; stride * height];
    for y in 0..height {
        for x in 0..width {
            let (r, g, b) = if x < width / 3 { ((x * 255 / width) as u32, (y * 255 / height) as u32, 128) }
                            else if x < 2 * width / 3 { if y < height / 2 { (30, 30, 46) } else { (166, 227, 161) } }
                            else { (255 - (y * 200 / height) as u32, 64, (x * 255 / width) as u32) };
            pixels[y * stride + x] = r << 16 | g << 8 | b;
        }
    }
    pixels
}

#[test]
fn tables_are_the_standard_ones() {
    for (counts, symbols) in [jpeg::DC_LUMA, jpeg::DC_CHROMA, jpeg::AC_LUMA, jpeg::AC_CHROMA] {
        assert_eq!(counts.iter().map(|&c| c as usize).sum::<usize>(), symbols.len());
        let mut seen = symbols.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), symbols.len(), "each symbol once");
    }
    assert_eq!(jpeg::AC_LUMA.1.len(), 162);
    assert_eq!(jpeg::AC_CHROMA.1.len(), 162);
    // Quality 50 is the standard table; 100 is all ones; lower quality divides more.
    let base = jpeg::quantization(&[16; 64], 50);
    assert_eq!(base[0], 16);
    assert!(jpeg::quantization(&[16; 64], 100).iter().all(|&q| q == 1));
    assert!(jpeg::quantization(&[16; 64], 10)[0] > 16);
    let mut sorted = jpeg::ZIGZAG.to_vec();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..64).collect::<Vec<_>>());
}

#[test]
fn encoded_pictures_decode_close_to_the_original() {
    for (width, height, stride, quality, psnr) in [(64, 48, 64, 90, 34.0), (100, 37, 128, 75, 28.0), (200, 120, 210, 80, 34.0), (1280, 16, 1280, 75, 28.0)] {
        let pixels = picture(width, height, stride);
        let mut out = Vec::new();
        jpeg::Encoder::new(quality).encode(&pixels, width, height, stride, &mut out);
        let (w, h, decoded) = decode(&out);
        assert_eq!((w, h), (width, height));
        let original: Vec<u32> = (0..height).flat_map(|y| pixels[y * stride..y * stride + width].to_vec()).collect();
        let (mean, ratio) = error(&original, &decoded);
        assert!(ratio > psnr && mean < 6.0, "{}x{} q{}: PSNR {:.1} dB, mean error {:.2}", width, height, quality, ratio, mean);
    }
    // Sizes that are not a multiple of 16: the blocks past the edge repeat the last row and column. (A smooth picture:
    // stripes of colour a few pixels wide lose most to the 4:2:0 chroma at any quality.)
    for (width, height, stride) in [(17, 9, 20), (33, 17, 40)] {
        let mut pixels = vec![0u32; stride * height];
        for y in 0..height { for x in 0..width { pixels[y * stride + x] = ((x * 255 / width) as u32) << 16 | ((y * 255 / height) as u32) << 8 | 100; } }
        let mut out = Vec::new();
        jpeg::Encoder::new(75).encode(&pixels, width, height, stride, &mut out);
        let (w, h, decoded) = decode(&out);
        assert_eq!((w, h), (width, height));
        let original: Vec<u32> = (0..height).flat_map(|y| pixels[y * stride..y * stride + width].to_vec()).collect();
        let (mean, ratio) = error(&original, &decoded);
        assert!(ratio > 28.0 && mean < 8.0, "{}x{}: PSNR {:.1} dB, mean error {:.2}", width, height, ratio, mean);
    }
    // A flat picture is small and comes back exactly (as near as rounding allows).
    let flat = vec![0x1E1E2E; 1280 * 800];
    let mut out = Vec::new();
    jpeg::Encoder::new(75).encode(&flat, 1280, 800, 1280, &mut out);
    assert!(out.len() < 40 * 1024, "a flat screen: {} bytes", out.len());
    let (_, _, decoded) = decode(&out);
    assert!(decoded.iter().all(|&p| channels(p).iter().zip(channels(0x1E1E2E)).all(|(a, b)| (a - b).abs() <= 2)));
}

#[test]
fn avi_header_chunks_and_index() {
    let mut index = avi::Index::default();
    let (first, chunk) = index.add(1001);
    assert_eq!((first, &chunk[..4], u32::from_le_bytes(chunk[4..].try_into().unwrap())), (avi::HEADER, &b"00dc"[..], 1001));
    let (second, _) = index.add(0); // a repeated frame
    assert_eq!(second, avi::HEADER + 8 + 1002, "an odd frame is padded to an even size");
    let (third, _) = index.add(500);
    assert_eq!(third, second + 8);
    assert_eq!(index.frames, [(4, 1001), (4 + 1010, 0), (4 + 1018, 500)]);
    assert_eq!(index.end(), third + 8 + 500);
    let header = avi::header(1280, 800, 10, &index, true);
    assert_eq!(header.len(), avi::HEADER);
    let word = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
    assert_eq!(&header[..4], b"RIFF");
    assert_eq!(word(4) as usize, index.end() + 8 + 16 * 3 - 8, "the RIFF size: the whole file but its first 8 bytes");
    assert_eq!(&header[8..12], b"AVI ");
    assert_eq!((&header[12..16], word(16), &header[20..28]), (&b"LIST"[..], 192, &b"hdrlavih"[..]));
    assert_eq!((word(32), word(36), word(48), word(56), word(64), word(68)), (100_000, 10 * 1001, 3, 1, 1280, 800), "avih");
    assert_eq!(word(44), 0x10, "AVIF_HASINDEX");
    assert_eq!(&header[88..92], b"LIST");
    assert_eq!(&header[96..112], b"strlstrh\x38\0\0\0vids");
    assert_eq!(&header[112..116], b"MJPG");
    assert_eq!((word(128), word(132), word(140)), (1, 10, 3), "scale, rate, length");
    assert_eq!(&header[164..168], b"strf");
    assert_eq!(&header[188..192], b"MJPG");
    assert_eq!(&header[212..216], b"LIST");
    assert_eq!(word(216), 4 + index.movi);
    assert_eq!(&header[avi::MOVI..avi::HEADER], b"movi");
    let idx1 = index.index();
    assert_eq!((&idx1[..4], idx1.len()), (&b"idx1"[..], 8 + 48));
    assert_eq!(&idx1[8..12], b"00dc");
    assert_eq!((u32::from_le_bytes(idx1[12..16].try_into().unwrap()), u32::from_le_bytes(idx1[28..32].try_into().unwrap())), (0x10, 0), "a repeated frame is not a key frame");
    assert_eq!(avi::header(1280, 800, 10, &index, false)[44], 0, "no AVIF_HASINDEX before the index is written");
}

#[test]
fn a_frame_like_the_last_one_codes_only_the_rows_that_changed() {
    // Issue 093: a restart marker ends each row of 16×16 blocks, so `record` codes only the rows whose pixels changed
    // (a clock's digits) and takes the others from the last frame; the file is the same as if all were coded again.
    let (width, height, stride) = (200, 120, 210);
    let first = picture(width, height, stride);
    let mut second = first.clone();
    for y in 40..50 { for x in 10..60 { second[y * stride + x] = 0xFFFFFF; } } // lines 40-49: rows 2 and 3
    let encoder = jpeg::Encoder::new(80);
    let mut rows = jpeg::Rows::default();
    let mut out = Vec::new();
    assert_eq!(encoder.encode_again(&first, None, width, height, stride, &mut rows, &mut out), 8, "the first frame: every row");
    let mut fresh = Vec::new();
    encoder.encode(&first, width, height, stride, &mut fresh);
    assert_eq!(out, fresh);
    out.clear();
    assert_eq!(encoder.encode_again(&second, Some(&first), width, height, stride, &mut rows, &mut out), 2, "rows 2 and 3 again");
    fresh.clear();
    encoder.encode(&second, width, height, stride, &mut fresh);
    assert_eq!(out, fresh, "the same file as coding every row");
    out.clear();
    assert_eq!(encoder.encode_again(&second, Some(&second), width, height, stride, &mut rows, &mut out), 0);
    assert_eq!(out, fresh);
    let (_, _, decoded) = decode(&out);
    assert!(channels(decoded[45 * width + 30]).iter().all(|&c| c > 235), "the white patch is there");
}
