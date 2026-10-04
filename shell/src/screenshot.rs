//! `screenshot [file]`: the screen in front as a 24-bit BMP (issue 086), through the shell's compositor client in
//! SLOT_DISPLAY (idl/display.wit: a sealed read-only copy of the pixels) and the shell's own VFS client. Without a file:
//! the first free `ram:screen-NNN.bmp`.
use crate::bmp;
use crate::console::Console;
use crate::files;
use core::fmt::Write;
use mind::abi::SLOT_DISPLAY;
use mind::fs::File;
use mind::idl::display;
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};
use mind::util::FixedBuf;

const DISPLAY: Endpoint = Endpoint(SLOT_DISPLAY);

fn text(name: &FixedBuf<64>) -> &str { core::str::from_utf8(name.as_bytes()).unwrap_or("") }
/// File writes go out in blocks of this size.
const CHUNK: usize = 64 * 1024;

/// `receive`: a free slot of the shell for the copy (dropped afterwards).
pub fn command(out: &mut Console, args: &[u8], receive: usize) {
    let mut name = FixedBuf::<64>::new();
    let given = core::str::from_utf8(args.trim_ascii()).unwrap_or("");
    if given.contains(char::is_whitespace) { let _ = writeln!(out, "USAGE: SCREENSHOT [FILE]"); return; }
    if given.is_empty() {
        let free = (1..1000).find(|n| { name.clear(); let _ = write!(name, "ram:screen-{:03}.bmp", n); mind::fs::metadata(text(&name)).is_err() });
        if free.is_none() { let _ = writeln!(out, "ERROR: SCREENSHOT: NO FREE NAME ON RAM:"); return; }
    } else {
        let _ = write!(name, "{}", given);
    }
    let path = text(&name);
    let Ok(mode) = display::mode(DISPLAY) else { let _ = writeln!(out, "ERROR: SCREENSHOT: NO COMPOSITOR"); return };
    match display::capture(DISPLAY, receive) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => { let _ = writeln!(out, "ERROR: SCREENSHOT: {}", if error == display::Error::NoScreen { "NOTHING ON THE SCREEN" } else { "NO MEMORY" }); return; }
        Err(_) => { let _ = writeln!(out, "ERROR: SCREENSHOT: NO COMPOSITOR"); return; }
    }
    let result = save(path, &mode, receive);
    let _ = mind::ipc::drop_cap(receive);
    match result {
        Ok(bytes) => { let _ = writeln!(out, "SCREENSHOT {}: {}x{}, {} BYTES", path, mode.width, mode.height, bytes); }
        Err(error) => { let _ = writeln!(out, "ERROR: SCREENSHOT: {}: {}", path, files::text(error)); }
    }
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
