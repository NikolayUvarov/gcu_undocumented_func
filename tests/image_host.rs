//! Host tests of the picture decoders for `wm`'s background (000-APP-0050): DEFLATE and zlib (libmind/src/inflate.rs),
//! PNG (png.rs) and baseline JPEG (jpegdec.rs), on files Pillow wrote (tests/data/images/make.py) of known procedural
//! pictures: lossless ones exactly, JPEG within an error bound, and damaged or cut files refused without a panic.
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/inflate.rs"]
mod inflate;
#[path = "../libmind/src/png.rs"]
mod png;
#[path = "../libmind/src/jpeg.rs"]
mod jpeg;
#[path = "../libmind/src/jpegdec.rs"]
mod jpegdec;

const W: usize = 40;
const H: usize = 24;

// From the repository's root, where scripts/host_tests.sh runs the tests.
fn file(name: &str) -> Vec<u8> { std::fs::read(format!("tests/data/images/{}", name)).unwrap() }

// The pictures the files hold (tests/data/images was written from these).
fn small(x: usize, y: usize) -> u32 { (((x * 6) & 255) << 16 | ((y * 10) & 255) << 8 | ((x + y) * 4) & 255) as u32 }
fn big(x: usize, y: usize) -> u32 {
    let n = (x * 7919 + y * 104_729 + x * y * 31) % 251;
    (((x * 3 + n / 8) & 255) << 16 | ((y * 5 + n / 4) & 255) << 8 | n & 255) as u32
}
fn grey(p: u32) -> u32 { let (r, g, b) = (p >> 16 & 255, p >> 8 & 255, p & 255); (r * 299 + g * 587 + b * 114 + 500) / 1000 }

fn png(name: &str, under: u32) -> Result<(usize, usize, Vec<u32>), &'static str> {
    let data = file(name);
    let mut pixels = Vec::new();
    let (w, h) = png::decode(&data, under, 4096, &mut |y, row| { assert_eq!(y * row.len(), pixels.len(), "rows in order"); pixels.extend_from_slice(row); })?;
    Ok((w, h, pixels))
}

fn jpeg(name: &str) -> Result<(usize, usize, Vec<u32>), &'static str> {
    let data = file(name);
    let mut pixels = Vec::new();
    let (w, h) = jpegdec::decode(&data, 4096, &mut |y, row| { assert_eq!(y * row.len(), pixels.len(), "rows in order"); pixels.extend_from_slice(row); })?;
    Ok((w, h, pixels))
}

// The mean and the largest difference of the channels.
fn error(got: &[u32], want: impl Fn(usize) -> u32) -> (f64, u32) {
    let (mut sum, mut max) = (0u64, 0u32);
    for (i, &p) in got.iter().enumerate() {
        let q = want(i);
        for s in [16, 8, 0] { let d = (p >> s & 255).abs_diff(q >> s & 255); sum += d as u64; max = max.max(d); }
    }
    (sum as f64 / (got.len() * 3) as f64, max)
}

#[test]
fn zlib_streams_of_every_block_kind_decompress() {
    // Fixed Huffman codes.
    let mut out = Vec::new();
    inflate::zlib(&file("fixed.zlib"), &mut |piece| { out.extend_from_slice(piece); true }).unwrap();
    let mut want = b"MIND Core ".repeat(50);
    want.extend(0..=255u8);
    assert_eq!(out, want);
    // A damaged checksum is found; a sink that stops ends it early without an error.
    let mut damaged = file("fixed.zlib");
    *damaged.last_mut().unwrap() ^= 1;
    assert_eq!(inflate::zlib(&damaged, &mut |_| true), Err("the checksum does not match"));
    let mut pieces = 0;
    inflate::zlib(&file("fixed.zlib"), &mut |_| { pieces += 1; false }).unwrap();
    assert_eq!(pieces, 1);
    assert!(inflate::zlib(b"not zlib", &mut |_| true).is_err());
}

#[test]
fn png_pictures_decode_exactly() {
    for name in ["rgb.png", "rgba.png"] {
        let (w, h, pixels) = png(name, 0).unwrap();
        assert_eq!((w, h), (W, H));
        assert_eq!(error(&pixels, |i| small(i % W, i / W)), (0.0, 0), "{}", name);
    }
    let (_, _, pixels) = png("grey.png", 0).unwrap();
    assert!(error(&pixels, |i| { let g = grey(small(i % W, i / W)); g << 16 | g << 8 | g }).1 <= 1);
    // 16 bits a sample: the high byte.
    let (_, _, pixels) = png("grey16.png", 0).unwrap();
    assert!(pixels.iter().enumerate().all(|(i, &p)| { let v = ((i % W * 1500 + i / W * 100) >> 8) as u32; p == v << 16 | v << 8 | v }));
    // A 4-bit palette of 16 colours, close to the picture.
    let (_, _, pixels) = png("palette.png", 0).unwrap();
    let mut colours: Vec<u32> = pixels.clone();
    colours.sort();
    colours.dedup();
    assert!(colours.len() <= 16 && error(&pixels, |i| small(i % W, i / W)).0 < 20.0);
    // Dynamic Huffman codes and stored blocks: the same larger picture.
    let (bw, bh, compressed) = png("big.png", 0).unwrap();
    let (_, _, stored) = png("stored.png", 0).unwrap();
    assert_eq!((bw, bh), (160, 96));
    assert_eq!(compressed, stored);
    assert_eq!(error(&compressed, |i| big(i % 160, i / 160)), (0.0, 0));
    assert_eq!(png::size(&file("big.png")), Some((160, 96)));
    // The compressed data split over many IDAT chunks, cut anywhere (the header and the checksum too): the same.
    for size in [1, 7, 500] {
        let data = rechunk(&file("big.png"), size);
        let mut pixels = Vec::new();
        png::decode(&data, 0, 4096, &mut |_, row| pixels.extend_from_slice(row)).unwrap();
        assert_eq!(pixels, compressed, "IDAT chunks of {} bytes", size);
    }
}

// A PNG with its IDAT data in chunks of `size` bytes (their CRCs left zero: the decoder does not read them).
fn rechunk(data: &[u8], size: usize) -> Vec<u8> {
    let (mut out, mut idat, mut at) = (data[..8].to_vec(), Vec::new(), 8);
    let chunk = |out: &mut Vec<u8>, kind: &[u8], body: &[u8]| { out.extend((body.len() as u32).to_be_bytes()); out.extend(kind); out.extend(body); out.extend([0; 4]); };
    while at + 12 <= data.len() {
        let len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let (kind, body) = (&data[at + 4..at + 8], &data[at + 8..at + 8 + len]);
        if kind == b"IDAT" { idat.extend_from_slice(body); } else {
            if kind == b"IEND" { for piece in idat.chunks(size) { chunk(&mut out, b"IDAT", piece); } }
            chunk(&mut out, kind, body);
        }
        at += 12 + len;
    }
    out
}

#[test]
fn baseline_jpeg_pictures_decode_close_to_the_original() {
    for (name, mean) in [("plain.jpg", 4.0), ("444.jpg", 3.0), ("restart.jpg", 4.0)] {
        let (w, h, pixels) = jpeg(name).unwrap();
        assert_eq!((w, h), (W, H));
        let (e, max) = error(&pixels, |i| small(i % W, i / W));
        assert!(e < mean && max < 64, "{}: mean {:.2}, largest {}", name, e, max);
    }
    let (_, _, pixels) = jpeg("grey.jpg").unwrap();
    let (e, _) = error(&pixels, |i| { let g = grey(small(i % W, i / W)); g << 16 | g << 8 | g });
    assert!(e < 3.0, "{}", e);
    // Against libjpeg's own decoding of each file (Pillow's, written beside it as RGB bytes). Full-resolution colour
    // and grey match but for IDCT rounding; 4:2:0 differs by libjpeg's smoothed chroma where this decoder repeats
    // each chroma sample, most on the noise picture (big.jpg).
    for (name, mean, largest) in [("big444.jpg", 0.2, 4), ("444.jpg", 0.2, 4), ("grey.jpg", 0.2, 2), ("plain.jpg", 2.5, 12), ("restart.jpg", 2.5, 12), ("big.jpg", 9.0, 128)] {
        let (_, _, pixels) = jpeg(name).unwrap();
        let reference = file(&format!("{}.rgb", name));
        let (e, max) = error(&pixels, |i| (reference[i * 3] as u32) << 16 | (reference[i * 3 + 1] as u32) << 8 | reference[i * 3 + 2] as u32);
        assert!(e < mean && max <= largest, "{}: mean {:.2}, largest {}", name, e, max);
    }
    let (bw, bh, _) = jpeg("big.jpg").unwrap();
    assert_eq!((bw, bh), (160, 96));
    assert_eq!(jpegdec::size(&file("big.jpg")), Some((160, 96)));
    assert_eq!(jpeg("progressive.jpg").err(), Some("a progressive or lossless JPEG"));
    // Flat halves on block edges (the wm suite's pictures): the colours themselves.
    let halves = |i: usize| if i % 64 < 32 { 0xC08040 } else { 0x4080C0 };
    assert_eq!(error(&jpeg("halves.jpg").unwrap().2, halves).1, 0);
    assert_eq!(error(&png("halves.png", 0).unwrap().2, halves).1, 0);
}

#[test]
fn damaged_pictures_are_refused_without_a_panic() {
    for name in ["rgb.png", "big.png", "palette.png", "plain.jpg", "restart.jpg", "big.jpg"] {
        let data = file(name);
        for cut in (0..data.len()).step_by((data.len() / 37).max(1)) {
            let short = &data[..cut];
            let _ = png::decode(short, 0, 4096, &mut |_, _| {});
            let _ = jpegdec::decode(short, 4096, &mut |_, _| {});
        }
        // Bytes changed here and there.
        let mut changed = data.clone();
        for i in (20..changed.len()).step_by(13) { changed[i] = changed[i].wrapping_mul(7).wrapping_add(3); }
        let _ = png::decode(&changed, 0, 4096, &mut |_, _| {});
        let _ = jpegdec::decode(&changed, 4096, &mut |_, _| {});
    }
    // Too large for the limit.
    assert_eq!(png("big.png", 0).map(|p| p.0).ok(), Some(160));
    assert!(png::decode(&file("big.png"), 0, 100, &mut |_, _| {}).is_err());
    assert!(jpegdec::decode(&file("big.jpg"), 100, &mut |_, _| {}).is_err());
}

// ---- 158-DRV-0084: a camera's Motion JPEG frames. ----

// A JPEG without its Huffman tables (DHT segments), as a UVC camera may send its frames.
fn without_tables(data: &[u8]) -> Vec<u8> {
    let mut out = data[..2].to_vec();
    let mut at = 2;
    while at + 4 <= data.len() && data[at] == 0xFF {
        let (marker, len) = (data[at + 1], u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize);
        if marker != 0xC4 { out.extend_from_slice(&data[at..at + 2 + len]); }
        at += 2 + len;
        if marker == 0xDA { out.extend_from_slice(&data[at..]); break; }
    }
    out
}

// The start of a JPEG's entropy-coded data: past its scan header.
fn scan_start(data: &[u8]) -> usize {
    let mut at = 2;
    loop {
        let (marker, len) = (data[at + 1], u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize);
        at += 2 + len;
        if marker == 0xDA { return at; }
    }
}

fn encoded(w: usize, h: usize, quality: u8) -> Vec<u8> {
    let pixels: Vec<u32> = (0..w * h).map(|i| big(i % w, i / w)).collect();
    let mut out = Vec::new();
    jpeg::Encoder::new(quality).encode(&pixels, w, h, w, &mut out);
    out
}

fn decoded(data: &[u8]) -> Result<(usize, usize, Vec<u32>), &'static str> {
    let mut pixels = Vec::new();
    let (w, h) = jpegdec::decode(data, 4096, &mut |y, row| { assert_eq!(y * row.len(), pixels.len(), "rows in order"); pixels.extend_from_slice(row); })?;
    Ok((w, h, pixels))
}

// Canonical Huffman codes from a table's counts and symbols: symbol → (code, length).
fn codes(table: &([u8; 16], &[u8])) -> std::collections::HashMap<u8, (u32, u32)> {
    let (mut map, mut code, mut k) = (std::collections::HashMap::new(), 0u32, 0usize);
    for len in 1..=16u32 {
        for _ in 0..table.0[len as usize - 1] { map.insert(table.1[k], (code, len)); code += 1; k += 1; }
        code <<= 1;
    }
    map
}

// A writer of entropy-coded bits: 0xFF stuffed with 0x00, the last byte padded with ones.
struct Writer { out: Vec<u8>, acc: u32, count: u32 }
impl Writer {
    fn put(&mut self, value: u32, length: u32) {
        for i in (0..length).rev() {
            self.acc = self.acc << 1 | (value >> i & 1);
            self.count += 1;
            if self.count == 8 { let b = self.acc as u8; self.out.push(b); if b == 0xFF { self.out.push(0); } (self.acc, self.count) = (0, 0); }
        }
    }
    fn finish(mut self) -> Vec<u8> { while self.count != 0 { self.put(1, 1); } self.out }
}

// A 4:2:2 frame (Y two blocks wide, Cb and Cr one) of flat blocks, with no Huffman tables, as UVC cameras send theirs:
// each 8 × 8 luma block's level and each 16 × 8 area's chroma given, every AC coefficient zero, all quantizers 1.
fn frame_422(w: usize, h: usize, luma: impl Fn(usize, usize) -> u8, chroma: impl Fn(usize, usize) -> (u8, u8)) -> Vec<u8> {
    let (dc_l, ac_l, dc_c, ac_c) = (codes(&jpeg::DC_LUMA), codes(&jpeg::AC_LUMA), codes(&jpeg::DC_CHROMA), codes(&jpeg::AC_CHROMA));
    let mut wr = Writer { out: Vec::new(), acc: 0, count: 0 };
    let mut pred = [0i32; 3];
    let mut block = |wr: &mut Writer, c: usize, sample: u8, dc: &std::collections::HashMap<u8, (u32, u32)>, ac: &std::collections::HashMap<u8, (u32, u32)>| {
        let value = (sample as i32 - 128) * 8;
        let diff = value - pred[c];
        pred[c] = value;
        let size = 32 - diff.unsigned_abs().leading_zeros();
        let (code, len) = dc[&(size as u8)];
        wr.put(code, len);
        wr.put(if diff >= 0 { diff as u32 } else { (diff + (1 << size) - 1) as u32 }, size);
        let (eob, len) = ac[&0];
        wr.put(eob, len);
    };
    for my in 0..h / 8 {
        for mx in 0..w / 16 {
            block(&mut wr, 0, luma(mx * 2, my), &dc_l, &ac_l);
            block(&mut wr, 0, luma(mx * 2 + 1, my), &dc_l, &ac_l);
            let (cb, cr) = chroma(mx, my);
            block(&mut wr, 1, cb, &dc_c, &ac_c);
            block(&mut wr, 2, cr, &dc_c, &ac_c);
        }
    }
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xDB, 0, 67, 0];
    out.extend([1u8; 64]);
    out.extend([0xFF, 0xC0, 0, 17, 8, (h >> 8) as u8, h as u8, (w >> 8) as u8, w as u8, 3, 1, 0x21, 0, 2, 0x11, 0, 3, 0x11, 0]);
    out.extend([0xFF, 0xDA, 0, 12, 3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    out.extend(wr.finish());
    out.extend([0xFF, 0xD9]);
    out
}

#[test]
fn a_frame_without_huffman_tables_takes_the_standard_ones() {
    let data = encoded(96, 64, 80);
    let bare = without_tables(&data);
    assert!(bare.len() < data.len() && !bare.windows(2).any(|w| w == [0xFF, 0xC4]), "the tables are gone");
    assert_eq!(decoded(&bare).unwrap(), decoded(&data).unwrap(), "the same picture as with the tables written");
}

#[test]
fn a_4_2_2_frame_without_tables_decodes_to_its_blocks() {
    let (w, h) = (48, 24);
    let luma = |bx: usize, by: usize| (16 + bx * 37 + by * 61) as u8;
    let chroma = |cx: usize, cy: usize| ((64 + cx * 50) as u8, (200 - cy * 40) as u8);
    let (gw, gh, pixels) = decoded(&frame_422(w, h, luma, chroma)).unwrap();
    assert_eq!((gw, gh), (w, h));
    for y in 0..h {
        for x in 0..w {
            let (l, (cb, cr)) = (luma(x / 8, y / 8) as f64, chroma(x / 16, y / 8));
            let (cb, cr) = (cb as f64 - 128.0, cr as f64 - 128.0);
            // JFIF's full-range YCbCr, independent of the decoder's integers.
            let want = [l + 1.402 * cr, l - 0.344_136 * cb - 0.714_136 * cr, l + 1.772 * cb].map(|v| v.round().clamp(0.0, 255.0) as i32);
            let p = pixels[y * w + x];
            let got = [(p >> 16 & 255) as i32, (p >> 8 & 255) as i32, (p & 255) as i32];
            for c in 0..3 { assert!((got[c] - want[c]).abs() <= 1, "({x}, {y}): {got:?} for {want:?}"); }
        }
    }
}

#[test]
fn the_inverse_dct_rounds_as_libjpeg_does() {
    // Against Pillow's (libjpeg's) own decoding of full-resolution pictures: its integer IDCT is the one used here, so
    // only the colour conversion's rounding is left.
    for name in ["444.jpg", "big444.jpg", "halves.jpg"] {
        let Ok(reference) = std::fs::read(format!("tests/data/images/{}.rgb", name)) else { continue };
        let (_, _, pixels) = jpeg(name).unwrap();
        let (e, max) = error(&pixels, |i| (reference[i * 3] as u32) << 16 | (reference[i * 3 + 1] as u32) << 8 | reference[i * 3 + 2] as u32);
        assert!(e < 0.1 && max <= 1, "{}: mean {:.3}, largest {}", name, e, max);
    }
    let (_, _, pixels) = jpeg("grey.jpg").unwrap();
    let reference = file("grey.jpg.rgb");
    assert!(pixels.iter().enumerate().all(|(i, &p)| p & 255 == reference[i * 3] as u32), "greyscale exactly as libjpeg");
}

#[test]
fn a_frame_cut_short_is_refused() {
    for data in [encoded(64, 48, 75), without_tables(&encoded(40, 24, 90)), frame_422(48, 24, |x, y| (x * 20 + y * 30) as u8, |x, _| (100 + x as u8, 150))] {
        let start = scan_start(&data);
        assert!(decoded(&data).is_ok());
        assert!(decoded(&data[..data.len() - 2]).is_ok(), "the end marker alone missing: every block is there");
        for cut in start..data.len() - 2 {
            let got = decoded(&data[..cut]);
            assert!(got.is_err(), "cut at {cut} of {}: decoded", data.len());
        }
        // A frame from a camera that lost a payload: a piece from the middle missing.
        let mut lost = data[..start + (data.len() - start) / 3].to_vec();
        lost.extend_from_slice(&data[start + (data.len() - start) / 2..]);
        let _ = decoded(&lost);
    }
}

#[test]
fn hostile_values_are_refused_without_overflow() {
    // A DC difference of 15 bits: the standard DC table has no such symbol, so a table that has one is sent.
    // Its one code, a single 0 bit, is how the frame's first DC begins (a difference of 0 is coded 00).
    let mut data = frame_422(16, 8, |_, _| 128, |_, _| (128, 128));
    let mut table = vec![0xFF, 0xC4, 0, 20, 0x00, 1];
    table.extend([0u8; 15]);
    table.push(15);
    let sos = data.windows(2).rposition(|w| w == [0xFF, 0xDA]).unwrap();
    data.splice(sos..sos, table);
    assert_eq!(decoded(&data).err(), Some("a DC difference of more than 11 bits"));
    // 16-bit quantizers of 65535 and the largest values: clamped, no overflow.
    let mut big_q = vec![0xFF, 0xD8, 0xFF, 0xDB, 0, 131, 0x10];
    big_q.extend([0xFFu8; 128]);
    let frame = frame_422(16, 8, |_, _| 255, |_, _| (0, 255));
    let after_dqt = 4 + 2 + 65;
    big_q.extend_from_slice(&frame[after_dqt..]);
    let (_, _, pixels) = decoded(&big_q).unwrap();
    assert_eq!(pixels.len(), 16 * 8);
}

/// Seeded fuzzing of the decoder with the frames a camera may send (for ASR's list, 158-DRV-0084): mutated pictures,
/// with and without their tables, never panic, and what decodes keeps its size. A run is evidence of the inputs it
/// made, not a proof (MC-12.2). MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the fixed seed and count.
#[test]
fn mutated_frames_never_panic() {
    let seed: u64 = std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(158_0084);
    let rounds: usize = std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut state = seed | 1;
    let mut next = move || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
    let originals = [encoded(48, 32, 80), without_tables(&encoded(32, 16, 50)), frame_422(32, 16, |x, y| (x * 40 + y * 9) as u8, |x, y| ((x * 90) as u8, (y * 70) as u8)),
        file("restart.jpg"), file("444.jpg"), file("grey.jpg"), without_tables(&file("plain.jpg"))];
    let (mut decodes, mut refused) = (0, 0);
    for i in 0..rounds {
        let mut bytes = originals[i % originals.len()].clone();
        for _ in 0..(1 + next() % 6) {
            let at = (next() as usize) % bytes.len();
            match next() % 5 { 0 => bytes[at] ^= 1 << (next() % 8), 1 => bytes[at] = next() as u8, 2 => bytes.truncate(at.max(1)), 3 => bytes.insert(at, next() as u8), _ => { bytes.remove(at); if bytes.is_empty() { bytes.push(0xFF); } } }
        }
        let mut rows = 0usize;
        match jpegdec::decode(&bytes, 256, &mut |y, row| { assert_eq!(y, rows, "rows in order"); assert!(row.len() <= 256); rows += 1; }) {
            Ok((w, h)) => { decodes += 1; assert!(w <= 256 && h == rows); }
            Err(_) => refused += 1,
        }
    }
    assert!(decodes > 0 && refused > 0, "the fuzzer reached both outcomes ({decodes} decoded, {refused} refused)");
}
