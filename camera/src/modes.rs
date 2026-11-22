//! The camera's modes in `camera` (158-APP-0152, `video.wit` 1.1's `modes` and `opened`): `-m <n>` picks one the
//! gateway offers, `m` steps to the next offered one and Shift+`m` to the one before, round the list; the mode in use in
//! words. No system calls: tests/camera_host.rs.
use alloc::format;
use alloc::string::String;

/// `video.wit` 1.1's `encoding`, by its code.
pub fn encoding(code: usize) -> &'static str { ["YUY2", "MJPEG", "UNCOMPRESSED", "SYNTHETIC"].get(code).copied().unwrap_or("UNKNOWN") }

/// A mode in words: `640x480 at 30/s YUY2`.
pub fn describe(encoding_code: usize, width: u16, height: u16, rate: u8) -> String { format!("{width}x{height} at {rate}/s {}", encoding(encoding_code)) }

/// The mode `-m` names, checked against the camera's list and what the gateway offers now.
pub fn chosen(index: u16, offered: &[bool], camera: usize) -> Result<usize, String> {
    let i = index as usize;
    match offered.len() {
        0 => Err(format!("camera {camera}: the gateway lists no modes")),
        n if i >= n => Err(format!("mode {index}: the gateway lists {n} modes for camera {camera}, numbers 0 to {}", n - 1)),
        _ if !offered[i] => Err(format!("mode {index} of camera {camera} is listed but not offered now")),
        _ => Ok(i),
    }
}

/// The offered mode after `current` (before it, with `forward` false), round the list; from the first offered one (or
/// the last) when the mode in use is none of the list's. None when no other mode is offered.
pub fn step(offered: &[bool], current: Option<usize>, forward: bool) -> Option<usize> {
    let n = offered.len();
    if n == 0 { return None; }
    let start = match current { Some(c) if c < n => c, _ => if forward { n - 1 } else { 0 } };
    (1..=n).map(|k| if forward { (start + k) % n } else { (start + n - k % n) % n })
        .find(|&i| offered[i] && Some(i) != current)
}
