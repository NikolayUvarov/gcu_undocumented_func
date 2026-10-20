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
    let frame = Framebuffer { base: info.fb_ptr, width: info.width, height: info.height, stride: info.stride, format: info.pixel_format, masks: info.pixel_masks };
    let Some(bytes) = frame.bytes() else { return; };
    if crate::mmu::reach_device(frame.base as usize, bytes) { activate(frame, bytes); }
}

/// Uses only an ABI 10/11 prefix and the inherited UEFI mapping, before allocator or kernel page tables exist.
pub unsafe fn init_early(info: *const BootInfo) {
    let frame = Framebuffer {
        base: core::ptr::addr_of!((*info).fb_ptr).read(), width: core::ptr::addr_of!((*info).width).read(),
        height: core::ptr::addr_of!((*info).height).read(), stride: core::ptr::addr_of!((*info).stride).read(),
        format: core::ptr::addr_of!((*info).pixel_format).read(), masks: core::ptr::addr_of!((*info).pixel_masks).read(),
    };
    if let Some(bytes) = frame.early_bytes() { activate(frame, bytes); }
}

#[derive(Clone, Copy)]
struct Framebuffer { base: *mut u32, width: usize, height: usize, stride: usize, format: u32, masks: [u32; 3] }
impl Framebuffer {
    fn bytes(self) -> Option<usize> {
        if self.base.is_null() || (self.base as usize) % 4 != 0 || self.width < 8 || self.height < 16 || self.width > self.stride || self.format > crate::abi::PIXEL_BITMASK { return None; }
        let bytes = self.stride.checked_mul(self.height)?.checked_mul(4)?;
        (self.base as usize).checked_add(bytes)?;
        Some(bytes)
    }
    fn early_bytes(self) -> Option<usize> {
        let bytes = self.bytes()?;
        // Larger early displays stay serial-only; no page-table walks, mapping changes or unbounded clears.
        if bytes > 256 * 1024 * 1024 || (self.base as u64).checked_add(bytes as u64)? > 1u64 << 32 { return None; }
        Some(bytes)
    }
}

unsafe fn activate(frame: Framebuffer, bytes: usize) {
    WIDTH.store(frame.width, Relaxed); HEIGHT.store(frame.height, Relaxed); STRIDE.store(frame.stride, Relaxed);
    FORMAT.store(frame.format, Relaxed);
    for (mask, &value) in MASKS.iter().zip(frame.masks.iter()) { mask.store(value, Relaxed); }
    BASE.store(frame.base as usize, Relaxed);
    let clear = pixel_to_device(BOOT.1, frame.format, frame.masks);
    for y in 0..frame.height { for x in 0..frame.width { core::ptr::write_volatile(frame.base.add(y * frame.stride + x), clear); } }
    crate::cpu::write_back(frame.base as usize, bytes);
}

#[cfg(test)]
mod tests {
    use super::Framebuffer;
    fn frame() -> Framebuffer { Framebuffer { base: 0x1000 as *mut u32, width: 640, height: 480, stride: 640, format: crate::abi::PIXEL_BGR, masks: [0; 3] } }
    #[test]
    fn early_framebuffer_rejects_invalid_geometry_and_overflow() {
        let f = frame();
        assert_eq!(f.early_bytes(), Some(640 * 480 * 4));
        for bad in [Framebuffer { base: core::ptr::null_mut(), ..f }, Framebuffer { base: 0x1001 as *mut u32, ..f }, Framebuffer { width: 7, ..f }, Framebuffer { height: 15, ..f }, Framebuffer { stride: 639, ..f }, Framebuffer { stride: usize::MAX, ..f }, Framebuffer { base: (usize::MAX - 3) as *mut u32, ..f }, Framebuffer { format: 3, ..f }] { assert_eq!(bad.early_bytes(), None); }
    }
    #[test]
    fn early_framebuffer_stays_within_its_mapping_and_clear_limit() {
        let f = frame();
        let bytes = f.bytes().unwrap();
        assert_eq!(Framebuffer { base: ((1u64 << 32) - bytes as u64) as *mut u32, ..f }.early_bytes(), Some(bytes));
        assert_eq!(Framebuffer { base: ((1u64 << 32) - bytes as u64 + 4) as *mut u32, ..f }.early_bytes(), None);
        assert_eq!(Framebuffer { width: 16384, stride: 16384, height: 4097, ..f }.early_bytes(), None);
    }
}

/// The compositor draws its first frame over the whole screen: the kernel writes there again only for a fatal stop.
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

/// The last lines of a halt a task asked for (000-KRN-0110): from the top of the screen as a fatal report, in the boot
/// colours; drawn once no other CPU runs a task that draws.
pub fn last(text: &str) { begin_fatal(); draw(text, BOOT); }

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
