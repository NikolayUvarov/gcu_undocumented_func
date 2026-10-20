//! `screenshot [-d seconds] [file]`: the screen in front as a 24-bit BMP (issue 086), through the shell's compositor
//! client in SLOT_DISPLAY (idl/display.wit: a sealed read-only copy of the pixels) and the shell's own VFS client.
//! Without a file: the first free `data/media/screen-NNN.bmp`, kept across reboots, or `ram:` when `data/` cannot be
//! written (000-APP-0089). `-d` takes it that many seconds later, the shell going on meanwhile; Ctrl+Alt+F6 takes one
//! at once whatever program is in front.
use crate::bmp;
use crate::console::Console;
use crate::files;
use alloc::string::String;
use core::fmt::Write;
use mind::abi::SLOT_DISPLAY;
use mind::fs::File;
use mind::idl::display;
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};

const DISPLAY: Endpoint = Endpoint(SLOT_DISPLAY);
/// File writes go out in blocks of this size.
const CHUNK: usize = 64 * 1024;
pub const USAGE: &str = "USAGE: SCREENSHOT [-D SECONDS] [FILE]";
/// The longest wait `-d` takes, in seconds.
const DELAY_MAX: u64 = 600;

/// What `screenshot` was asked: a file (empty: a free name) and the wait before it, in ms.
pub struct Asked { pub file: String, pub delay_ms: u64 }

pub fn parse(args: &[u8]) -> Result<Asked, &'static str> {
    let mut asked = Asked { file: String::new(), delay_ms: 0 };
    let mut words = core::str::from_utf8(args).unwrap_or("").split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "-d" | "-D" => asked.delay_ms = words.next().and_then(|n| n.parse::<u64>().ok()).filter(|n| (1..=DELAY_MAX).contains(n)).ok_or(USAGE)? * 1000,
            _ if !asked.file.is_empty() || word.starts_with('-') => return Err(USAGE),
            _ => asked.file = String::from(word),
        }
    }
    Ok(asked)
}

/// The first free `<stem>-NNN.<extension>` in `data/media/`, made if need be; on `ram:` when `data/` cannot be written.
pub fn free_name(stem: &str, extension: &str) -> Option<String> {
    let directory = if mind::fs::mkdir("data/media").is_ok() { "data/media/" } else { "ram:" };
    (1..1000).map(|n| alloc::format!("{}{}-{:03}.{}", directory, stem, n, extension)).find(|name| mind::fs::metadata(name).is_err())
}

/// Takes the screen in front into `path` (empty: a free name): the line to show, `SCREENSHOT <path>: WxH, N BYTES`, or
/// an error's. `receive`: a free slot of the shell for the copy (dropped afterwards).
pub fn take(path: &str, receive: usize) -> String {
    let named;
    let path = if path.is_empty() {
        match free_name("screen", "bmp") { Some(name) => { named = name; named.as_str() } None => return String::from("ERROR: SCREENSHOT: NO FREE NAME IN DATA/MEDIA/ OR ON RAM:") }
    } else { path };
    let Ok(mode) = display::mode(DISPLAY) else { return String::from("ERROR: SCREENSHOT: NO COMPOSITOR") };
    match display::capture(DISPLAY, receive) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return alloc::format!("ERROR: SCREENSHOT: {}", if error == display::Error::NoScreen { "NOTHING ON THE SCREEN" } else { "NO MEMORY" }),
        Err(_) => return String::from("ERROR: SCREENSHOT: NO COMPOSITOR"),
    }
    let result = save(path, &mode, receive);
    let _ = mind::ipc::drop_cap(receive);
    match result {
        Ok(bytes) => alloc::format!("SCREENSHOT {}: {}x{}, {} BYTES", path, mode.width, mode.height, bytes),
        Err(error) => alloc::format!("ERROR: SCREENSHOT: {}: {}", path, files::text(error)),
    }
}

/// `screenshot` taken now, from the console `out`.
pub fn command(out: &mut Console, path: &str, receive: usize) {
    // The console draws lazily: show the command line as typed before the copy is taken.
    out.render(None);
    let line = take(path, receive);
    let _ = writeln!(out, "{}", line);
}

// Writes the copy in slot `copy` as a BMP to `path`; returns the file's size.
fn save(path: &str, mode: &display::Mode, copy: usize) -> Result<usize, mind::fs::Error> {
    let (width, height, stride) = (mode.width as usize, mode.height as usize, mode.stride as usize);
    let screen = Mapping::new(copy).map_err(|_| mind::fs::Error::NoMemory)?;
    if width == 0 || stride < width || screen.len() < stride * height * 4 { return Err(mind::fs::Error::Invalid); }
    let pixels = unsafe { core::slice::from_raw_parts(screen.as_ptr::<u32>() as *const u32, stride * height) };
    let mut buffer = Pages::new(CHUNK.max(bmp::row_bytes(width) + bmp::HEADER)).ok_or(mind::fs::Error::NoMemory)?;
    let buffer = buffer.as_mut_slice();
    let mut file = File::create(path)?;
    buffer[..bmp::HEADER].copy_from_slice(&bmp::header(width, height));
    let mut filled = bmp::HEADER;
    for y in (0..height).rev() {
        if filled + bmp::row_bytes(width) > buffer.len() { file.write(&buffer[..filled])?; filled = 0; }
        filled += bmp::row(&pixels[y * stride..y * stride + width], &mut buffer[filled..]);
    }
    file.write(&buffer[..filled])?;
    file.flush()?;
    Ok(bmp::file_bytes(width, height))
}
