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
