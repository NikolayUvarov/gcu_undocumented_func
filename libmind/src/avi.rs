//! An AVI file with one Motion JPEG stream (issue 093): the header before the frames, the frames as `00dc` chunks,
//! the `idx1` index after them. The header has a fixed size, so a writer puts it first with the counts it has, appends
//! the frames, and writes it again with the final counts. A frame of no bytes repeats the one before (players show the
//! previous frame for it). No system calls: tests/jpeg_host.rs.
use alloc::vec::Vec;

/// Bytes before the first frame: RIFF, the `hdrl` list (avih, strl with strh and strf) and the `movi` list's header.
pub const HEADER: usize = 224;
/// Where the `movi` list's fourcc is; `idx1` offsets count from it (the first frame is at offset 4).
pub const MOVI: usize = HEADER - 4;

/// The frames written so far: their offsets (from `movi`) and sizes, for the index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index { pub frames: Vec<(u32, u32)>, pub movi: u32, pub largest: u32 }

impl Index {
    /// A frame of `len` bytes is appended: where its chunk goes in the file, and the chunk's 8-byte header (its data,
    /// then a pad byte when `len` is odd, follow).
    pub fn add(&mut self, len: u32) -> (usize, [u8; 8]) {
        let at = HEADER + self.movi as usize;
        self.frames.push((self.movi + 4, len));
        self.movi += 8 + len + (len & 1);
        self.largest = self.largest.max(len);
        let mut chunk = [0u8; 8];
        chunk[..4].copy_from_slice(b"00dc");
        chunk[4..].copy_from_slice(&len.to_le_bytes());
        (at, chunk)
    }

    /// The `idx1` chunk: a key frame each, a repeated one (no bytes) not.
    pub fn index(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 16 * self.frames.len());
        out.extend_from_slice(b"idx1");
        out.extend_from_slice(&(16 * self.frames.len() as u32).to_le_bytes());
        for &(offset, len) in &self.frames {
            out.extend_from_slice(b"00dc");
            out.extend_from_slice(&(if len > 0 { 0x10u32 } else { 0 }).to_le_bytes()); // AVIIF_KEYFRAME
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
        }
        out
    }

    /// Where `idx1` goes: after the last frame.
    pub fn end(&self) -> usize { HEADER + self.movi as usize }
}

fn put(out: &mut Vec<u8>, words: &[u32]) { for w in words { out.extend_from_slice(&w.to_le_bytes()); } }

/// The header for `width` × `height` frames at `fps`, with the frames of `index` (and, when `indexed`, the `idx1`
/// chunk after them counted in the RIFF size).
pub fn header(width: u32, height: u32, fps: u32, index: &Index, indexed: bool) -> Vec<u8> {
    let frames = index.frames.len() as u32;
    let idx1 = if indexed { 8 + 16 * frames } else { 0 };
    let riff = (HEADER as u32 - 8) + index.movi + idx1;
    let mut out = Vec::with_capacity(HEADER);
    out.extend_from_slice(b"RIFF");
    put(&mut out, &[riff]);
    out.extend_from_slice(b"AVI LIST");
    put(&mut out, &[192]);
    out.extend_from_slice(b"hdrlavih");
    // avih: µs per frame, max bytes per second, padding, flags (AVIF_HASINDEX), frames, initial frames, streams,
    // suggested buffer, width, height, four reserved.
    put(&mut out, &[56, 1_000_000 / fps.max(1), index.largest * fps, 0, if indexed { 0x10 } else { 0 }, frames, 0, 1, index.largest, width, height, 0, 0, 0, 0]);
    out.extend_from_slice(b"LIST");
    put(&mut out, &[116]);
    out.extend_from_slice(b"strlstrh");
    put(&mut out, &[56]);
    out.extend_from_slice(b"vidsMJPG");
    // flags, priority and language, initial frames, scale, rate, start, length, suggested buffer, quality, sample size.
    put(&mut out, &[0, 0, 0, 1, fps, 0, frames, index.largest, u32::MAX, 0]);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(height as u16).to_le_bytes());
    out.extend_from_slice(b"strf");
    // BITMAPINFOHEADER: size, width, height, planes and bit count, compression, image size, two densities, colours.
    put(&mut out, &[40, 40, width, height, 1 | 24 << 16]);
    out.extend_from_slice(b"MJPG");
    put(&mut out, &[width * height * 3, 0, 0, 0, 0]);
    out.extend_from_slice(b"LIST");
    put(&mut out, &[4 + index.movi]);
    out.extend_from_slice(b"movi");
    debug_assert_eq!(out.len(), HEADER);
    out
}
