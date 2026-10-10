//! USB video class (issue 158): what `video_gw` needs to drive a UVC camera through `usb_host` — the camera's
//! descriptors (formats, frame sizes, the streaming interface's alternate settings), the probe and commit of a format,
//! and frames assembled from payloads. No system calls: tests/uvc_host.rs.
use alloc::vec::Vec;

/// The interface class and subclasses of a camera.
pub const CLASS_VIDEO: u8 = 0x0E;
pub const SUBCLASS_CONTROL: u8 = 1;
pub const SUBCLASS_STREAMING: u8 = 2;

/// Class requests SET_CUR and GET_CUR, and the streaming interface's probe and commit controls (wValue's high byte).
pub const SET_CUR: u8 = 0x01;
pub const GET_CUR: u8 = 0x81;
pub const PROBE: u8 = 1;
pub const COMMIT: u8 = 2;

const CS_INTERFACE: u8 = 0x24;
const VC_HEADER: u8 = 1;
const VS_INPUT_HEADER: u8 = 1;
const VS_FORMAT_UNCOMPRESSED: u8 = 4;
const VS_FRAME_UNCOMPRESSED: u8 = 5;
const VS_FORMAT_MJPEG: u8 = 6;
const VS_FRAME_MJPEG: u8 = 7;

/// YUY2's GUID in an uncompressed format descriptor.
pub const GUID_YUY2: [u8; 16] = [0x59, 0x55, 0x59, 0x32, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71];

/// How a format's frames are coded: YUY2, Motion JPEG, or another uncompressed one (its GUID's first four bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding { Yuy2, Mjpeg, Other([u8; 4]) }

/// One frame size of a format, with the indexes the probe names it by. Intervals are in 100 ns units: a list, or with a
/// nonzero `step` the continuous range `intervals[0]..=intervals[1]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSize {
    pub format: u8, pub encoding: Encoding, pub frame: u8, pub width: u16, pub height: u16,
    pub max_bytes: u32, pub default_interval: u32, pub intervals: Vec<u32>, pub step: u32,
}

impl FrameSize {
    /// The interval to ask for a rate of at least 10^7 / `wanted` a second: the longest that is no longer, else the shortest.
    pub fn interval(&self, wanted: u32) -> u32 {
        if self.step != 0 {
            let (min, max) = (self.intervals[0], self.intervals[1].max(self.intervals[0]));
            let w = wanted.clamp(min, max);
            return min + (w - min) / self.step * self.step;
        }
        let list = self.intervals.iter().copied();
        list.clone().filter(|&i| i <= wanted).max().or_else(|| list.min()).unwrap_or(self.default_interval)
    }
    /// Its highest rate, frames a second.
    pub fn max_rate(&self) -> u32 {
        let shortest = if self.step != 0 { self.intervals[0] } else { self.intervals.iter().copied().min().unwrap_or(self.default_interval) };
        10_000_000u32.checked_div(shortest).unwrap_or(0)
    }
    /// The bytes of one frame when the encoding fixes them (YUY2: two a pixel).
    pub fn exact_bytes(&self) -> Option<usize> { (self.encoding == Encoding::Yuy2).then_some(self.width as usize * self.height as usize * 2) }
}

/// An alternate setting of the streaming interface with its isochronous IN endpoint: its address, wMaxPacketSize as the
/// descriptor gives it (bits 11-12: the extra transactions a microframe) and bInterval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alternate { pub setting: u8, pub endpoint: u8, pub packet: u16, pub interval: u8 }

impl Alternate {
    /// The bytes it moves an interval.
    pub fn bytes(&self) -> u32 { (self.packet & 0x7FF) as u32 * ((self.packet >> 11 & 3) as u32 + 1) }
}

/// A camera as its configuration descriptor describes it: the first streaming interface and its formats.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Camera {
    /// bcdUVC of the control interface's header (0x0100, 0x0110, 0x0150).
    pub version: u16,
    /// The control and streaming interfaces' numbers.
    pub control: u8, pub streaming: u8,
    /// The streaming endpoint: the input header's, or a bulk IN endpoint of alternate setting 0 (`bulk`).
    pub endpoint: u8, pub bulk: bool,
    pub formats: Vec<(u8, Encoding)>, pub frames: Vec<FrameSize>, pub alternates: Vec<Alternate>,
}

/// The camera a configuration descriptor describes, or None if it has no control and streaming interface with a frame size.
pub fn parse(config: &[u8]) -> Option<Camera> {
    let mut camera = Camera::default();
    let (mut control, mut streaming) = (false, false);
    // The interface the descriptors at hand belong to: its number, alternate setting and video subclass (0: another class).
    let (mut number, mut alternate, mut subclass) = (0u8, 0u8, 0u8);
    let mut format: Option<(u8, Encoding)> = None;
    let mut at = 0;
    while at + 2 <= config.len() {
        let length = config[at] as usize;
        if length < 2 || at + length > config.len() { break; }
        let d = &config[at..at + length];
        let word = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        let ours = |n: u8| streaming && subclass == SUBCLASS_STREAMING && n == camera.streaming;
        match d[1] {
            4 if length >= 9 => {
                (number, alternate) = (d[2], d[3]);
                subclass = if d[5] == CLASS_VIDEO { d[6] } else { 0 };
                if subclass == SUBCLASS_CONTROL && !control { control = true; camera.control = number; }
                if subclass == SUBCLASS_STREAMING && !streaming && alternate == 0 { streaming = true; camera.streaming = number; }
            }
            CS_INTERFACE if subclass == SUBCLASS_CONTROL && number == camera.control && length >= 5 && d[2] == VC_HEADER => camera.version = u16::from_le_bytes([d[3], d[4]]),
            CS_INTERFACE if ours(number) && length >= 3 => match d[2] {
                VS_INPUT_HEADER if length >= 7 => camera.endpoint = d[6],
                VS_FORMAT_UNCOMPRESSED if length >= 21 => {
                    let encoding = if d[5..21] == GUID_YUY2 { Encoding::Yuy2 } else { Encoding::Other([d[5], d[6], d[7], d[8]]) };
                    format = Some((d[3], encoding)); camera.formats.push((d[3], encoding));
                }
                VS_FORMAT_MJPEG if length >= 5 => { format = Some((d[3], Encoding::Mjpeg)); camera.formats.push((d[3], Encoding::Mjpeg)); }
                kind @ (VS_FRAME_UNCOMPRESSED | VS_FRAME_MJPEG) if length >= 26 => {
                    let Some((index, encoding)) = format.filter(|f| (f.1 == Encoding::Mjpeg) == (kind == VS_FRAME_MJPEG)) else { at += length; continue };
                    let default_interval = word(21);
                    let (mut intervals, step) = match d[25] as usize {
                        0 if length >= 38 => (alloc::vec![word(26), word(30)], word(34).max(1)),
                        0 => (alloc::vec![default_interval, default_interval], 1),
                        n => ((0..n).filter(|i| 30 + 4 * i <= length).map(|i| word(26 + 4 * i)).collect(), 0),
                    };
                    if intervals.is_empty() { intervals.push(default_interval); }
                    camera.frames.push(FrameSize { format: index, encoding, frame: d[3], width: u16::from_le_bytes([d[5], d[6]]), height: u16::from_le_bytes([d[7], d[8]]),
                        max_bytes: word(17), default_interval, intervals, step });
                }
                // Another kind of format (frame-based, a stream of H.264) has frames of its own that are not used.
                0x10 | 0x12 => format = None,
                _ => {}
            },
            5 if length >= 7 && ours(number) => {
                let (address, attributes, packet, interval) = (d[2], d[3], u16::from_le_bytes([d[4], d[5]]), d[6]);
                if attributes & 3 == 1 && address & 0x80 != 0 && alternate != 0 { camera.alternates.push(Alternate { setting: alternate, endpoint: address, packet, interval }); }
                if attributes & 3 == 2 && address & 0x80 != 0 && alternate == 0 { camera.bulk = true; camera.endpoint = address; }
            }
            _ => {}
        }
        at += length;
    }
    (control && streaming && !camera.frames.is_empty()).then_some(camera)
}

impl Camera {
    fn yuy2(&self) -> impl Iterator<Item = &FrameSize> + Clone { self.frames.iter().filter(|f| f.encoding == Encoding::Yuy2) }

    /// The frame size to stream for a picture of `width` × `height`: of the YUY2 ones (what the gateway converts), the
    /// smallest that holds it, else the largest.
    pub fn choose(&self, width: u16, height: u16) -> Option<&FrameSize> {
        let area = |f: &&FrameSize| f.width as u32 * f.height as u32;
        self.yuy2().filter(|f| f.width >= width && f.height >= height).min_by_key(area).or_else(|| self.yuy2().max_by_key(area))
    }

    /// The largest YUY2 frame size no wider than `limit` (the smallest if all are wider): what the gateway offers.
    pub fn largest(&self, limit: u16) -> Option<&FrameSize> {
        let area = |f: &&FrameSize| f.width as u32 * f.height as u32;
        self.yuy2().filter(|f| f.width <= limit).max_by_key(area).or_else(|| self.yuy2().min_by_key(area))
    }

    /// The alternate setting to stream with: the one moving the fewest bytes an interval that still carries `bytes`
    /// (the committed dwMaxPayloadTransferSize), else the one moving the most.
    pub fn alternate(&self, bytes: u32) -> Option<Alternate> {
        let list = self.alternates.iter().copied();
        list.clone().filter(|a| a.bytes() >= bytes).min_by_key(Alternate::bytes).or_else(|| list.max_by_key(Alternate::bytes))
    }
}

/// The length of the probe and commit controls: 26 bytes for UVC 1.0, 34 for 1.1, 48 for 1.5.
pub fn probe_length(version: u16) -> usize { if version >= 0x0150 { 48 } else if version >= 0x0110 { 34 } else { 26 } }

/// The probe and commit controls (UVC 1.5, 4.3.1.1) as bytes: what the gateway asks for and the camera's answer, which
/// is committed as it came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe { pub bytes: [u8; 48], pub length: usize }

impl Probe {
    /// A probe of `format` and `frame` at `interval`, which the camera is asked to keep (bmHint bit 0).
    pub fn new(version: u16, format: u8, frame: u8, interval: u32) -> Self {
        let mut bytes = [0u8; 48];
        bytes[0] = 1;
        bytes[2] = format; bytes[3] = frame;
        bytes[4..8].copy_from_slice(&interval.to_le_bytes());
        Self { bytes, length: probe_length(version) }
    }
    pub fn from_bytes(source: &[u8]) -> Self {
        let mut bytes = [0u8; 48];
        let length = source.len().min(48);
        bytes[..length].copy_from_slice(&source[..length]);
        Self { bytes, length }
    }
    pub fn as_bytes(&self) -> &[u8] { &self.bytes[..self.length] }
    fn word(&self, at: usize) -> u32 { u32::from_le_bytes([self.bytes[at], self.bytes[at + 1], self.bytes[at + 2], self.bytes[at + 3]]) }
    pub fn format(&self) -> u8 { self.bytes[2] }
    pub fn frame(&self) -> u8 { self.bytes[3] }
    pub fn interval(&self) -> u32 { self.word(4) }
    /// dwMaxVideoFrameSize and dwMaxPayloadTransferSize.
    pub fn max_frame(&self) -> u32 { self.word(18) }
    pub fn max_payload(&self) -> u32 { self.word(22) }
}

/// Payload header bits (UVC 1.5, 2.4.3.3): the frame ID, toggled every frame; the end of a frame; an error.
pub const FID: u8 = 1;
pub const EOF: u8 = 2;
pub const ERR: u8 = 0x40;

/// Frames from payloads. A payload starts with a header of bHeaderLength bytes; its second byte holds FID, EOF and ERR.
/// A frame ends at EOF or when the frame ID changes. It is good when no payload of it was in error and it has the bytes
/// its encoding fixes (any, but some, for one that does not).
pub struct Assembler {
    filling: Vec<u8>, ready: Vec<u8>, capacity: usize, exact: bool, fid: Option<u8>, bad: bool,
    /// Frames ended good and broken, and the bytes of the last that ended.
    pub good: u32, pub broken: u32, pub last_bytes: usize,
}

impl Assembler {
    /// Frames of `bytes` bytes (`exact`) or of up to `bytes`.
    pub fn new(bytes: usize, exact: bool) -> Self {
        Self { filling: Vec::with_capacity(bytes), ready: Vec::with_capacity(bytes), capacity: bytes, exact, fid: None, bad: false, good: 0, broken: 0, last_bytes: 0 }
    }

    /// One payload; `damaged` when the host controller reported an error for it. Some(good) when a frame ended with it;
    /// the last good frame stays in `ready` until another good one ends.
    pub fn feed(&mut self, payload: &[u8], damaged: bool) -> Option<bool> {
        if payload.len() < 2 { return None; } // an empty packet carries nothing
        let header = payload[0] as usize;
        if header < 2 || header > payload.len() { self.bad = true; return None; }
        let info = payload[1];
        let mut ended = None;
        if self.fid.is_some_and(|f| f != info & FID) { ended = self.end(); }
        self.fid = Some(info & FID);
        if damaged || info & ERR != 0 { self.bad = true; }
        let data = &payload[header..];
        if self.filling.len() + data.len() > self.capacity { self.bad = true; } else { self.filling.extend_from_slice(data); }
        if info & EOF != 0 { ended = self.end().or(ended); }
        ended
    }

    // The frame so far ends; None if it had nothing (no bytes, no error).
    fn end(&mut self) -> Option<bool> {
        let bytes = self.filling.len();
        if bytes == 0 && !self.bad { return None; }
        let good = !self.bad && if self.exact { bytes == self.capacity } else { bytes > 0 };
        if good { core::mem::swap(&mut self.filling, &mut self.ready); self.good += 1; } else { self.broken += 1; }
        self.last_bytes = bytes;
        self.filling.clear();
        self.bad = false;
        Some(good)
    }

    /// The last good frame.
    pub fn ready(&self) -> &[u8] { &self.ready }
}
