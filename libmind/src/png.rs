//! PNG pictures (000-APP-0050's backgrounds) decoded a row at a time: greyscale, RGB, palette, with or without
//! alpha, 1 to 16 bits a sample, not interlaced. Alpha is laid over a colour the caller gives. Rows are handed to a
//! sink as `0x00RRGGBB` pixels as they come out of the zlib stream (`inflate`), read from the file's chunks where
//! they lie. No system calls: host-tested in tests/image_host.rs.
use crate::inflate;
use alloc::vec;
use alloc::vec::Vec;

pub type Error = &'static str;

/// The picture's width and height, from its header.
pub fn size(data: &[u8]) -> Option<(usize, usize)> {
    if data.len() < 24 || &data[..8] != b"\x89PNG\r\n\x1a\n" || &data[12..16] != b"IHDR" { return None; }
    let be = |at: usize| u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize;
    Some((be(16), be(20)))
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
}

/// Decodes `data`, handing each row (its number, its pixels) to `row`; alpha is laid over `under`. Refuses pictures
/// wider or taller than `limit` pixels.
pub fn decode(data: &[u8], under: u32, limit: usize, row: &mut dyn FnMut(usize, &[u32])) -> Result<(usize, usize), Error> {
    let (width, height) = size(data).ok_or("not a PNG")?;
    if data.len() < 33 { return Err("a PNG cut short"); }
    if width == 0 || height == 0 || width > limit || height > limit { return Err("too large a PNG"); }
    let (depth, colour, interlace) = (data[24], data[25], data[28]);
    if interlace != 0 { return Err("an interlaced PNG"); }
    let channels = match colour { 0 => 1, 2 => 3, 3 => 1, 4 => 2, 6 => 4, _ => return Err("a PNG of an unknown colour type") };
    if !matches!((colour, depth), (0, 1 | 2 | 4 | 8 | 16) | (3, 1 | 2 | 4 | 8) | (2 | 4 | 6, 8 | 16)) { return Err("a PNG of an unknown bit depth"); }
    // The chunks: the palette, its transparency, and the compressed data joined.
    let (mut palette, mut alpha, mut compressed): (&[u8], &[u8], Vec<&[u8]>) = (&[], &[], Vec::new());
    let mut at = 8;
    while at + 12 <= data.len() {
        let len = u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize;
        let kind = &data[at + 4..at + 8];
        let body = data.get(at + 8..at + 8 + len).ok_or("a chunk cut short")?;
        match kind { b"PLTE" => palette = body, b"tRNS" => alpha = body, b"IDAT" => compressed.push(body), b"IEND" => break, _ => {} }
        at += 12 + len;
    }
    if colour == 3 && palette.is_empty() { return Err("a palette PNG without its palette"); }
    let bits = channels * depth as usize;
    let stride = (width * bits).div_ceil(8);
    let step = bits.div_ceil(8); // bytes to the same byte of the pixel to the left
    let (mut previous, mut current) = (vec![0u8; stride], Vec::with_capacity(stride + 1));
    let mut pixels = vec![0u32; width];
    let mut y = 0;
    let blend = |c: u32, a: u32, shift: u32| ((c * a + ((under >> shift) & 0xFF) * (255 - a)) / 255) << shift;
    let mut bad = None;
    inflate::zlib_parts(&compressed, &mut |piece| {
        for &byte in piece {
            if y >= height { return false; }
            current.push(byte);
            if current.len() < stride + 1 { continue; }
            let filter = current[0];
            let line = &mut current[1..];
            for i in 0..stride {
                let (a, b, c) = (if i >= step { line[i - step] } else { 0 }, previous[i], if i >= step { previous[i - step] } else { 0 });
                line[i] = match filter {
                    0 => line[i], 1 => line[i].wrapping_add(a), 2 => line[i].wrapping_add(b),
                    3 => line[i].wrapping_add(((a as u16 + b as u16) / 2) as u8), 4 => line[i].wrapping_add(paeth(a, b, c)),
                    _ => { bad = Some("a row of an unknown filter"); return false; }
                };
            }
            // A sample's value scaled to 8 bits.
            let sample = |x: usize, k: usize| -> u32 {
                match depth {
                    16 => line[(x * channels + k) * 2] as u32,
                    8 => line[x * channels + k] as u32,
                    d => {
                        let d = d as usize;
                        let bit = (x * channels + k) * d;
                        let v = (line[bit / 8] >> (8 - d - bit % 8)) as u32 & ((1 << d) - 1);
                        if colour == 3 { v } else { v * 255 / ((1 << d) - 1) }
                    }
                }
            };
            for (x, p) in pixels.iter_mut().enumerate() {
                let (r, g, b, a) = match colour {
                    0 => { let v = sample(x, 0); (v, v, v, 255) }
                    2 => (sample(x, 0), sample(x, 1), sample(x, 2), 255),
                    3 => {
                        let i = sample(x, 0) as usize;
                        let c = palette.get(i * 3..i * 3 + 3).unwrap_or(&[0, 0, 0]);
                        (c[0] as u32, c[1] as u32, c[2] as u32, alpha.get(i).copied().unwrap_or(255) as u32)
                    }
                    4 => { let v = sample(x, 0); (v, v, v, sample(x, 1)) }
                    _ => (sample(x, 0), sample(x, 1), sample(x, 2), sample(x, 3)),
                };
                *p = blend(r, a, 16) | blend(g, a, 8) | blend(b, a, 0);
            }
            row(y, &pixels);
            previous.copy_from_slice(line);
            current.clear();
            y += 1;
        }
        y < height
    })?;
    if let Some(error) = bad { return Err(error); }
    if y < height { return Err("the picture's data ends early"); }
    Ok((width, height))
}
