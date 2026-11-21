// The video gateway's choices that need no device (158-DRV-0082): which camera a number names, whether a program may
// open it beside the streams held, and how many test patterns the boot disk asks for. No system calls:
// tests/video_gw_host.rs.

/// What a stream reads: test pattern `n` (from 0), or the USB camera of that streaming interface handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key { Pattern(u8), Camera(u32) }

/// Why an open is refused before a camera is touched: no such camera, or another program owns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused { NotFound, Busy }

/// How many USB cameras `cameras` lists: as many as there are, up to `listed` places less the patterns'.
pub fn real(usb: usize, patterns: usize, listed: usize) -> usize { usb.min(listed.saturating_sub(patterns)) }

/// The camera numbered `camera` in `cameras`' order: the USB cameras (their streaming handles, in order), then the
/// patterns.
pub fn key(camera: u8, usb: &[u32], patterns: usize, listed: usize) -> Option<Key> {
    let (n, real) = (camera as usize, real(usb.len(), patterns, listed));
    if n < real { Some(Key::Camera(usb[n])) } else if n - real < patterns.min(listed) { Some(Key::Pattern((n - real) as u8)) } else { None }
}

/// Whether `sender` may open `key` beside the streams `held` (owner and key each): refused while another owner holds
/// that camera (one owner a camera, video.wit), else the place in `held` of the sender's own earlier stream, which is
/// closed first (one stream a program: `read` and `close` name no camera). Another owner's stream of another camera
/// stays open beside it.
pub fn admit(held: &[(u64, Key)], sender: u64, key: Option<Key>) -> Result<Option<usize>, Refused> {
    let key = key.ok_or(Refused::NotFound)?;
    if held.iter().any(|&(owner, k)| owner != sender && k == key) { return Err(Refused::Busy); }
    Ok(held.iter().position(|&(owner, _)| owner == sender))
}

/// The test patterns `video/synthetic` asks for, from its first byte: none without the file, two when it is `2`, else one.
pub fn patterns(first: Option<Option<u8>>) -> usize {
    match first { None => 0, Some(Some(b'2')) => 2, Some(_) => 1 }
}

/// The frame of pattern `n` that a stream's frame `sequence` shows: the second pattern runs half a minute ahead, so two
/// streams of the two differ in their counters.
pub fn pattern_frame(n: u8, sequence: u32, rate: u8) -> u32 { if n == 0 { sequence } else { sequence.wrapping_add(30 * rate as u32) } }

/// The frame sizes and rates `open` takes for a USB camera (158-DRV-0083): what a frame's pages and the conversion
/// were sized for.
pub const WIDTHS: core::ops::RangeInclusive<u16> = 16..=1280;
pub const HEIGHTS: core::ops::RangeInclusive<u16> = 16..=1024;
pub const RATES: core::ops::RangeInclusive<u32> = 1..=60;

/// Whether `open` takes a USB camera at `width` × `height` and `rate`.
pub fn opens(width: u16, height: u16, rate: u32) -> bool { WIDTHS.contains(&width) && HEIGHTS.contains(&height) && RATES.contains(&rate) }

/// Whether the gateway offers a camera's mode: YUY2 (it decodes no other encoding yet, 158-DRV-0084), within what
/// `open` takes, and carried by the camera's streaming settings, which move `capacity` bytes a second at most (None:
/// a bulk camera, whose payloads take the bus as it comes).
pub fn offered(yuy2: bool, width: u16, height: u16, rate: u32, bytes_per_second: u64, capacity: Option<u64>) -> bool {
    yuy2 && opens(width, height, rate) && capacity.is_none_or(|c| bytes_per_second <= c)
}

/// The modes a test pattern lists (any width a multiple of 32 up to 640 is drawn; these are for the tests).
pub const PATTERN_MODES: [(u16, u16, u8); 3] = [(640, 480, 30), (320, 240, 10), (160, 120, 5)];
