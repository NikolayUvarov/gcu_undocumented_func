// Kernel text on the boot framebuffer (211-KRN-0013): the boot lines until a task takes the screen, and a fatal stop
// (panic, kernel exception, init's exit) over whatever is on it, for machines without a serial port.
#[allow(dead_code)]
#[path = "../../common/font16.rs"]
mod font16;

use crate::abi::{pixel_to_device, BootInfo};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Relaxed};

static BASE: AtomicUsize = AtomicUsize::new(0); // 0: no screen the kernel can reach
static WIDTH: AtomicUsize = AtomicUsize::new(0);
static HEIGHT: AtomicUsize = AtomicUsize::new(0);
static STRIDE: AtomicUsize = AtomicUsize::new(0);
static FORMAT: AtomicU32 = AtomicU32::new(0);
static MASKS: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
static TAKEN: AtomicBool = AtomicBool::new(false); // a task holds the framebuffer: boot lines stop
static FATAL: AtomicBool = AtomicBool::new(false); // a fatal report owns the screen from its first row
static ROW: AtomicUsize = AtomicUsize::new(0);
static COL: AtomicUsize = AtomicUsize::new(0);

const BOOT: (u32, u32) = (0x00C0_C0C0, 0x0000_0000); // grey on black
const STOP: (u32, u32) = (0x00FF_FFFF, 0x0080_0000); // white on dark red

/// Takes the bootloader's framebuffer if the kernel's identity map reaches it, and clears it.
pub unsafe fn init(info: &BootInfo) {
    let bytes = info.stride * info.height * 4;
    if info.fb_ptr.is_null() || info.width < 8 || info.height < 16 || !crate::mmu::reach_device(info.fb_ptr as usize, bytes) { return; }
    WIDTH.store(info.width, Relaxed); HEIGHT.store(info.height, Relaxed); STRIDE.store(info.stride, Relaxed);
    FORMAT.store(info.pixel_format, Relaxed);
    for (mask, &value) in MASKS.iter().zip(info.pixel_masks.iter()) { mask.store(value, Relaxed); }
    BASE.store(info.fb_ptr as usize, Relaxed);
    let clear = pixel_to_device(BOOT.1, info.pixel_format, info.pixel_masks);
    for y in 0..info.height { for x in 0..info.width { core::ptr::write_volatile((info.fb_ptr).add(y * info.stride + x), clear); } }
    crate::cpu::write_back(info.fb_ptr as usize, bytes);
}

/// A task was given the framebuffer (the compositor): the kernel writes there again only for a fatal stop.
pub fn take() { TAKEN.store(true, Relaxed); }

/// A boot line, while no task holds the screen.
pub fn print(text: &str) { if showing() { draw(text, BOOT); } }

/// Whether the boot lines are still on the screen: no task holds it and no fatal report owns it.
pub fn showing() -> bool { BASE.load(Relaxed) != 0 && !TAKEN.load(Relaxed) && !FATAL.load(Relaxed) }

/// A service's log bytes as boot lines (211-KRN-0017): UTF-8 as it is, other bytes as '?'.
pub fn print_bytes(bytes: &[u8]) {
    if !showing() { return; }
    let mut rest = bytes;
    while !rest.is_empty() {
        match core::str::from_utf8(rest) {
            Ok(text) => { draw(text, BOOT); break; }
            Err(error) => {
                let (good, bad) = rest.split_at(error.valid_up_to());
                draw(unsafe { core::str::from_utf8_unchecked(good) }, BOOT); draw("?", BOOT);
                rest = &bad[error.error_len().unwrap_or(bad.len()).min(bad.len())..];
            }
        }
    }
}

/// The start of a fatal report: below the boot lines while they are on the screen, else from its top, over whatever a
/// task drew there.
pub fn begin_fatal() {
    if FATAL.swap(true, Relaxed) { return; }
    if TAKEN.load(Relaxed) { ROW.store(0, Relaxed); } else if COL.load(Relaxed) != 0 { ROW.store((ROW.load(Relaxed) + 1) % (HEIGHT.load(Relaxed) / 16).max(1), Relaxed); }
    COL.store(0, Relaxed);
}

/// Text of a fatal report.
pub fn fatal(text: &str) { draw(text, STOP); }

fn draw(text: &str, (fg, bg): (u32, u32)) {
    let base = BASE.load(Relaxed) as *mut u32;
    if base.is_null() { return; }
    let (width, height, stride) = (WIDTH.load(Relaxed), HEIGHT.load(Relaxed), STRIDE.load(Relaxed));
    let (format, masks) = (FORMAT.load(Relaxed), [MASKS[0].load(Relaxed), MASKS[1].load(Relaxed), MASKS[2].load(Relaxed)]);
    let (fg, bg) = (pixel_to_device(fg, format, masks), pixel_to_device(bg, format, masks));
    // The 8x16 grid mind::tui and the tests' screen reader use: centred where the screen is not a multiple of a cell.
    let (cols, rows, x0, y0) = (width / 8, height / 16, width % 8 / 2, height % 16 / 2);
    let cell = |row: usize, col: usize, ch: char| unsafe {
        let code = u16::try_from(ch as u32).unwrap_or(0xFFFD);
        let index = font16::CODES.binary_search(&code).or_else(|_| font16::CODES.binary_search(&0x3F)).unwrap_or(0);
        for (r, bits) in font16::GLYPHS[index].iter().enumerate() {
            let line = base.add((y0 + row * 16 + r) * stride + x0 + col * 8);
            for c in 0..8 { core::ptr::write_volatile(line.add(c), if bits & (0x80 >> c) != 0 { fg } else { bg }); }
        }
    };
    let (first, mut row, mut col) = (ROW.load(Relaxed), ROW.load(Relaxed), COL.load(Relaxed));
    let mut wrapped = false;
    for ch in text.chars() {
        if ch == '\r' { continue; }
        if ch == '\n' || col == cols {
            for rest in col..cols { cell(row, rest, ' '); }
            row = (row + 1) % rows; col = 0; wrapped |= row == 0;
            if ch == '\n' { continue; }
        }
        cell(row, col, ch); col += 1;
    }
    for rest in col..cols { cell(row, rest, ' '); }
    ROW.store(row, Relaxed); COL.store(col, Relaxed);
    let (top, bottom) = if wrapped || row < first { (0, rows) } else { (first, row + 1) };
    unsafe { crate::cpu::write_back(base.add((y0 + top * 16) * stride) as usize, (bottom - top) * 16 * stride * 4); }
}
