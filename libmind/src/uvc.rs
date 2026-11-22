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
/// VS_STREAM_ERROR_CODE_CONTROL: why the camera last marked a payload in error (158-DRV-0082).
pub const STREAM_ERROR: u8 = 6;

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
    /// The buffer one of its frames is assembled in, and whether a frame must fill it exactly (158-DRV-0084): YUY2's two
    /// bytes a pixel; a Motion JPEG frame up to the committed dwMaxVideoFrameSize (`committed`, 0: the descriptor's), but
    /// three bytes a pixel at most and 1 KiB at least, so a camera's claim is not trusted further; another encoding up
    /// to its descriptor's size.
    pub fn buffer(&self, committed: u32) -> (usize, bool) {
        let pixels = self.width as usize * self.height as usize;
        match self.encoding {
            Encoding::Yuy2 => (pixels * 2, true),
            Encoding::Mjpeg => ((if committed == 0 { self.max_bytes } else { committed } as usize).min(pixels * 3).max(1024), false),
            Encoding::Other(_) => (self.max_bytes as usize, false),
        }
    }
    /// Its intervals as a camera's modes list them (158-DRV-0083): a list as given; a continuous range, its shortest and
    /// its longest (one when they are the same).
    pub fn listed(&self) -> Vec<u32> {
        if self.step == 0 { return self.intervals.clone(); }
        let (shortest, longest) = (self.intervals[0], self.intervals[1].max(self.intervals[0]));
        if shortest == longest { alloc::vec![shortest] } else { alloc::vec![shortest, longest] }
    }
}

/// One mode of a camera (158-DRV-0083): a frame size of a format at one of its frame intervals (100 ns units), with
/// the indexes the probe names it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode { pub format: u8, pub frame: u8, pub encoding: Encoding, pub width: u16, pub height: u16, pub interval: u32, pub max_bytes: u32 }

impl Mode {
    /// Its frames a second: 10^7 / interval rounded down, 0 for less than one a second.
    pub fn rate(&self) -> u32 { 10_000_000u32.checked_div(self.interval).unwrap_or(0) }
    /// The bytes a second its frames take: YUY2's two a pixel, another encoding's up to its frame descriptor's most.
    pub fn bytes_per_second(&self) -> u64 {
        let frame = if self.encoding == Encoding::Yuy2 { self.width as u64 * self.height as u64 * 2 } else { self.max_bytes as u64 };
        (frame * 10_000_000).checked_div(self.interval as u64).unwrap_or(u64::MAX)
    }
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
    // The frame sizes the gateway turns into pictures: YUY2, and Motion JPEG (158-DRV-0084).
    fn decoded(&self) -> impl Iterator<Item = &FrameSize> + Clone { self.frames.iter().filter(|f| matches!(f.encoding, Encoding::Yuy2 | Encoding::Mjpeg)) }

    /// Every mode (158-DRV-0083), in the order of the descriptors: each frame size of each format at each interval it
    /// lists. The same descriptors give the same list.
    pub fn modes(&self) -> Vec<Mode> {
        self.frames.iter().flat_map(|f| f.listed().into_iter().map(move |interval| Mode {
            format: f.format, frame: f.frame, encoding: f.encoding, width: f.width, height: f.height, interval, max_bytes: f.max_bytes,
        })).collect()
    }

    /// The most bytes a second its isochronous settings carry at `speed` (usb_host's: 1 full, 2 low, 3 high, 4 super),
    /// a payload header of 12 bytes an interval aside: an interval is a frame of 1 ms at full speed, a microframe of
    /// 125 µs at high, 2^(bInterval - 1) of them. None for a bulk camera, whose payloads take the bus as it comes, and
    /// above high speed, where the bursts of the SuperSpeed companion descriptor are not read.
    pub fn capacity(&self, speed: u8) -> Option<u64> {
        if self.bulk || speed >= 4 { return None; }
        let a_second: u64 = if speed == 3 { 8000 } else { 1000 };
        Some(self.alternates.iter().map(|a| a.bytes().saturating_sub(12) as u64 * (a_second >> (a.interval.clamp(1, 16) - 1))).max().unwrap_or(0))
    }

    /// The frame size to stream for a picture of `width` × `height` at `rate` a second, of those the gateway decodes
    /// (YUY2 and MJPEG, 158-DRV-0084): the smallest that holds the picture and reaches the rate, YUY2 before MJPEG at one
    /// size since it needs no decoding; else of those that hold it, the one of the highest rate, the smaller and YUY2
    /// first; else the largest.
    pub fn choose(&self, width: u16, height: u16, rate: u32) -> Option<&FrameSize> {
        let area = |f: &FrameSize| f.width as u32 * f.height as u32;
        let jpeg = |f: &FrameSize| f.encoding == Encoding::Mjpeg;
        let holding = self.decoded().filter(|f| f.width >= width && f.height >= height);
        holding.clone().filter(|f| f.max_rate() >= rate).min_by_key(|f| (area(f), jpeg(f)))
            .or_else(|| holding.max_by_key(|f| (f.max_rate(), core::cmp::Reverse(area(f)), !jpeg(f))))
            .or_else(|| self.decoded().max_by_key(|f| (area(f), !jpeg(f))))
    }

    /// Whether `open` at mode `mode`'s size and rate streams that mode's frame size (158-DRV-0084): a Motion JPEG mode
    /// whose size YUY2 also gives at its rate is streamed as YUY2 instead, so it is not one `open` reaches.
    pub fn opens_as(&self, mode: &Mode) -> bool {
        self.choose(mode.width, mode.height, mode.rate()).is_some_and(|f| f.format == mode.format && f.frame == mode.frame)
    }

    /// The largest YUY2 frame size no wider than `limit` (the smallest if all are wider): what the gateway offers. A
    /// camera without YUY2 offers its Motion JPEG sizes so.
    pub fn largest(&self, limit: u16) -> Option<&FrameSize> {
        let area = |f: &&FrameSize| f.width as u32 * f.height as u32;
        let yuy2 = self.yuy2().next().is_some();
        let all = || self.decoded().filter(move |f| !yuy2 || f.encoding == Encoding::Yuy2);
        all().filter(|f| f.width <= limit).max_by_key(area).or_else(|| all().min_by_key(area))
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

/// The bytes a frame buffer of frames that vary (Motion JPEG) starts at.
pub const GROWING: usize = 128 * 1024;

/// Payload header bits (UVC 1.5, 2.4.3.3): the frame ID, toggled every frame; the end of a frame; an error.
pub const FID: u8 = 1;
pub const EOF: u8 = 2;
pub const ERR: u8 = 0x40;

/// Frames from payloads. A payload starts with a header of bHeaderLength bytes; its second byte holds FID, EOF and ERR.
/// A frame ends at EOF or when the frame ID changes. It is good when no payload of it was in error and it has the bytes
/// its encoding fixes (any, but some, for one that does not).
pub struct Assembler {
    filling: Vec<u8>, ready: Vec<u8>, capacity: usize, exact: bool, fid: Option<u8>, bad: bool, why: u8,
    /// Frames ended good and broken, and the bytes of the last that ended.
    pub good: u32, pub broken: u32, pub last_bytes: usize,
    /// Why frames broke, and how every frame ended (158-DRV-0082).
    pub causes: Causes,
}

/// Why frames broke (a frame counts once for each cause it had: its payloads' error bit, a host controller's error, a
/// payload header that does not parse, more bytes than a frame holds, a bulk payload that started without a valid header
/// or whose transfer failed, or the wrong size alone), and how frames ended (a changed frame ID, or EOF).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Causes { pub error_bit: u32, pub damaged: u32, pub header: u32, pub overflow: u32, pub bulk_start: u32, pub bulk_failed: u32, pub size: u32, pub by_fid: u32, pub by_eof: u32 }

const WHY_ERROR: u8 = 1; const WHY_DAMAGED: u8 = 2; const WHY_HEADER: u8 = 4; const WHY_OVERFLOW: u8 = 8; const WHY_START: u8 = 16; const WHY_FAILED: u8 = 32;

impl Assembler {
    /// Frames of `bytes` bytes (`exact`) or of up to `bytes`.
    pub fn new(bytes: usize, exact: bool) -> Self {
        Self { filling: Vec::with_capacity(bytes), ready: Vec::with_capacity(bytes), capacity: bytes, exact, fid: None, bad: false, why: 0, good: 0, broken: 0, last_bytes: 0, causes: Causes::default() }
    }
    /// The same, None when the memory is not there (158-DRV-0084): frames of a fixed size get their two buffers whole;
    /// frames of up to `bytes` (Motion JPEG) start at GROWING bytes and grow as their frames do, since a camera's
    /// maximum is many times its frames (the Ryzen's: 1.8 MB for frames of 0.1 MB, and two streams' maxima did not fit).
    pub fn try_new(bytes: usize, exact: bool) -> Option<Self> {
        let first = if exact { bytes } else { bytes.min(GROWING) };
        let (mut filling, mut ready) = (Vec::new(), Vec::new());
        filling.try_reserve_exact(first).ok()?;
        ready.try_reserve_exact(first).ok()?;
        Some(Self { filling, ready, capacity: bytes, exact, fid: None, bad: false, why: 0, good: 0, broken: 0, last_bytes: 0, causes: Causes::default() })
    }

    /// One payload; `damaged` when the host controller reported an error for it. Some(good) when a frame ended with it;
    /// the last good frame stays in `ready` until another good one ends.
    pub fn feed(&mut self, payload: &[u8], damaged: bool) -> Option<bool> {
        if payload.len() < 2 { return None; } // an empty packet carries nothing
        let header = payload[0] as usize;
        if header < 2 || header > payload.len() { self.mark(WHY_HEADER); return None; }
        let info = payload[1];
        let ended = self.begin(info, damaged);
        self.data(&payload[header..]);
        self.finish(info).or(ended)
    }

    // A payload's header: a changed frame ID ends the frame before it; an error or damage spoils the frame.
    fn begin(&mut self, info: u8, damaged: bool) -> Option<bool> {
        let ended = if self.fid.is_some_and(|f| f != info & FID) { let e = self.end(); if e.is_some() { self.causes.by_fid += 1; } e } else { None };
        self.fid = Some(info & FID);
        if damaged { self.mark(WHY_DAMAGED); }
        if info & ERR != 0 { self.mark(WHY_ERROR); }
        ended
    }

    // A payload's bytes after its header; a frame past its capacity, or that finds no memory to grow, is spoiled.
    fn data(&mut self, data: &[u8]) {
        if self.filling.len() + data.len() > self.capacity || self.filling.try_reserve(data.len()).is_err() { self.mark(WHY_OVERFLOW); } else { self.filling.extend_from_slice(data); }
    }

    // A payload's end: EOF in its header ends the frame.
    fn finish(&mut self, info: u8) -> Option<bool> { if info & EOF == 0 { return None; } let e = self.end(); if e.is_some() { self.causes.by_eof += 1; } e }

    // The frame so far is spoiled, for `why`.
    fn mark(&mut self, why: u8) { self.bad = true; self.why |= why; }

    // The frame so far ends; None if it had nothing (no bytes, no error).
    fn end(&mut self) -> Option<bool> {
        let bytes = self.filling.len();
        if bytes == 0 && !self.bad { return None; }
        let good = !self.bad && if self.exact { bytes == self.capacity } else { bytes > 0 };
        if good { core::mem::swap(&mut self.filling, &mut self.ready); self.good += 1; } else {
            self.broken += 1;
            let (w, c) = (self.why, &mut self.causes);
            for (bit, count) in [(WHY_ERROR, &mut c.error_bit), (WHY_DAMAGED, &mut c.damaged), (WHY_HEADER, &mut c.header), (WHY_OVERFLOW, &mut c.overflow), (WHY_START, &mut c.bulk_start), (WHY_FAILED, &mut c.bulk_failed)] {
                if w & bit != 0 { *count += 1; }
            }
            if w == 0 { c.size += 1; }
        }
        self.last_bytes = bytes;
        self.filling.clear();
        (self.bad, self.why) = (false, 0);
        Some(good)
    }

    /// The last good frame.
    pub fn ready(&self) -> &[u8] { &self.ready }
    /// The most bytes a frame may have.
    pub fn capacity(&self) -> usize { self.capacity }
}

/// Payloads from a bulk endpoint (158-DRV-0072): a payload of up to `max` bytes (the committed dwMaxPayloadTransferSize)
/// may span several transfers, and its header is at the start of its first. It ends with a short transfer, or when it
/// reaches `max`.
pub struct Bulk { max: usize, at: usize, info: u8 }

impl Bulk {
    pub fn new(max: u32) -> Self { Self { max: (max as usize).max(2), at: 0, info: 0 } }

    /// How many bytes to ask the next transfer for, at most `limit`: no more than the payload has left.
    pub fn next(&self, limit: usize) -> usize { (self.max - self.at).min(limit).max(1) }

    /// One transfer of `asked` bytes that brought `data`. Some(good) when a frame ended with it.
    pub fn transfer(&mut self, assembler: &mut Assembler, data: &[u8], asked: usize) -> Option<bool> {
        let mut ended = None;
        let mut body = data;
        if self.at == 0 {
            if data.is_empty() { return None; } // a zero-length transfer between payloads
            let header = data[0] as usize;
            if data.len() < 2 || header < 2 || header > data.len() { assembler.mark(WHY_START); return None; }
            self.info = data[1];
            ended = assembler.begin(self.info, false);
            body = &data[header..];
        }
        assembler.data(body);
        self.at += data.len();
        if data.len() < asked || self.at >= self.max {
            self.at = 0;
            ended = assembler.finish(self.info).or(ended);
        }
        ended
    }

    /// A transfer that failed: the payload in progress is dropped, its frame spoiled, and the next transfer starts anew.
    pub fn failed(&mut self, assembler: &mut Assembler) { if self.at != 0 { assembler.mark(WHY_FAILED); } self.at = 0; }
}
