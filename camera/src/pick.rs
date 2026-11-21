//! Which camera `camera` shows (158-APP-0147): `-c <n>` picks one of those `cameras` lists, and `n` or Tab moves to the
//! next one, round from the last to the first. No system calls: tests/camera_host.rs.
use alloc::format;
use alloc::string::String;

/// The camera `-c` names, checked against how many the gateway lists.
pub fn chosen(index: u8, count: usize) -> Result<usize, String> {
    match count {
        0 => Err(String::from("no camera (the video gateway lists none)")),
        _ if (index as usize) < count => Ok(index as usize),
        1 => Err(format!("camera {}: the video gateway lists one camera, number 0", index)),
        _ => Err(format!("camera {}: the video gateway lists {} cameras, numbers 0 to {}", index, count, count - 1)),
    }
}

/// The camera after `current`, round to the first after the last.
pub fn next(current: usize, count: usize) -> usize { if count == 0 { 0 } else { (current + 1) % count } }

/// The window's title: the camera's number from 1, the count and its name.
pub fn title(index: usize, count: usize, name: &str) -> String { format!("camera {}/{}: {}", index + 1, count, name) }
