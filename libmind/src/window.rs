//! Window surfaces (idl/window.wit, issue 157): memory of the window broker, lent to the program that draws in it and
//! to the window manager that shows it. The program draws the content and says what changed; the manager copies the
//! content to the screen, asks for another size and queues input events. Each side writes only its own fields, and
//! each checks what it reads: the other side may be broken or hostile. No file here depends on the kernel: host tests
//! build it.
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// The badge of the broker's manager client (the shell lends it for `REQUEST_WINDOW_MANAGER`).
pub const BADGE_MANAGER: u16 = 1;
pub const MAGIC: u32 = 0x4E49_574D; // "MWIN"
pub const HEADER: usize = 4096;
pub const TITLE: usize = 64;
pub const EVENTS: usize = 64;
/// A text cell: the character, the foreground and the background colour (0x00RRGGBB), little-endian u32 each.
pub const CELL: usize = 12;
/// Largest surfaces: 256 × 128 cells, a 1920 × 1200 pixel screen.
pub const MAX_COLUMNS: usize = 256;
pub const MAX_ROWS: usize = 128;
pub const MAX_PIXELS: (usize, usize) = (1920, 1200);
pub const MAX_BYTES: usize = HEADER + 1920 * 1200 * 4;

/// What the content is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Text = 1, Pixels = 2 }

/// What the manager asks of the program (written by the manager and the broker).
pub const STATE_SHOWN: u32 = 0;
pub const STATE_HIDDEN: u32 = 1; // no manager shows it: drawing less is fine
pub const STATE_CLOSE: u32 = 2; // the window is closed: the program should end

// Offsets in the header. The program writes MAGIC..DAMAGE and the title, the manager WANTED, STATE and the event head.
const O_MAGIC: usize = 0;
const O_KIND: usize = 4;
const O_SIZE: usize = 8; // width u16, height u16 (cells or pixels)
const O_WANTED: usize = 12; // width u16, height u16 the manager asks for (0: no request)
const O_CHANGES: usize = 16; // u32, bumped by the program after drawing
const O_DAMAGE: usize = 20; // x, y, width, height u16: what the last changes touched (all 0: everything)
const O_TITLE: usize = 32;
const O_STATE: usize = 128;
const O_EVENT_HEAD: usize = 192; // events the manager queued
const O_EVENT_TAIL: usize = 256; // events the program took
const O_EVENTS: usize = 512; // EVENTS input event words (common/abi.rs layout), u64 each

/// The size a surface of `kind` with this many columns and rows (or pixels) needs.
pub fn bytes(kind: Kind, width: usize, height: usize) -> usize {
    HEADER + width * height * match kind { Kind::Text => CELL, Kind::Pixels => 4 }
}

/// A surface mapped at `base`, `len` bytes long; copying it copies the address only.
#[derive(Clone, Copy)]
pub struct Surface { base: *mut u8, len: usize }

impl Surface {
    /// # Safety
    /// `base` must map `len` writable bytes, aligned to 4096, for as long as the surface is used.
    pub unsafe fn new(base: *mut u8, len: usize) -> Self { Self { base, len } }

    fn u32(&self, at: usize) -> &AtomicU32 { unsafe { AtomicU32::from_ptr(self.base.add(at).cast::<u32>()) } }
    fn u64(&self, at: usize) -> &AtomicU64 { unsafe { AtomicU64::from_ptr(self.base.add(at).cast::<u64>()) } }
    fn pair(&self, at: usize) -> (usize, usize) { let v = self.u32(at).load(Ordering::Acquire); ((v & 0xFFFF) as usize, (v >> 16) as usize) }
    fn set_pair(&self, at: usize, a: usize, b: usize) { self.u32(at).store((a.min(0xFFFF) | b.min(0xFFFF) << 16) as u32, Ordering::Release); }

    // Program side.

    /// Writes a fresh header (the broker, before it lends the surface).
    pub fn init(&self, kind: Kind, width: usize, height: usize, title: &str) {
        unsafe { core::ptr::write_bytes(self.base, 0, HEADER); }
        self.u32(O_KIND).store(kind as u32, Ordering::Relaxed);
        self.set_pair(O_SIZE, width, height);
        self.set_title(title);
        self.u32(O_MAGIC).store(MAGIC, Ordering::Release);
    }
    pub fn set_title(&self, title: &str) {
        let mut end = title.len().min(TITLE);
        while !title.is_char_boundary(end) { end -= 1; }
        let mut bytes = [0u8; TITLE]; bytes[..end].copy_from_slice(&title.as_bytes()[..end]);
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), self.base.add(O_TITLE), TITLE); }
    }
    /// After a resize the program draws at its new size (it must fit the surface's memory).
    pub fn set_size(&self, width: usize, height: usize) -> bool {
        let fits = self.kind().is_some_and(|kind| bytes(kind, width, height) <= self.len);
        if fits { self.set_pair(O_SIZE, width, height); }
        fits
    }
    /// The program drew `damage` (x, y, width, height; None: everything).
    pub fn changed(&self, damage: Option<(usize, usize, usize, usize)>) {
        let (x, y, w, h) = damage.unwrap_or((0, 0, 0, 0));
        self.set_pair(O_DAMAGE, x, y); self.set_pair(O_DAMAGE + 4, w, h);
        self.u32(O_CHANGES).fetch_add(1, Ordering::Release);
    }
    /// The content: cells or pixels, `bytes` of it.
    pub fn content(&self) -> *mut u8 { unsafe { self.base.add(HEADER) } }
    /// The size the manager asks for, once (it is cleared).
    pub fn wanted(&self) -> Option<(usize, usize)> {
        let v = self.u32(O_WANTED).swap(0, Ordering::AcqRel);
        (v != 0).then_some(((v & 0xFFFF) as usize, (v >> 16) as usize))
    }
    /// The next input event the manager queued (common/abi.rs word).
    pub fn event(&self) -> Option<usize> {
        let tail = self.u32(O_EVENT_TAIL).load(Ordering::Relaxed);
        let head = self.u32(O_EVENT_HEAD).load(Ordering::Acquire);
        let queued = head.wrapping_sub(tail) as usize;
        if queued == 0 || queued > EVENTS { return None; } // empty, or a head nobody could have written
        let word = self.u64(O_EVENTS + 8 * (tail as usize % EVENTS)).load(Ordering::Relaxed) as usize;
        self.u32(O_EVENT_TAIL).store(tail.wrapping_add(1), Ordering::Release);
        Some(word)
    }

    // Manager and broker side.

    /// The header's kind and size, if the program wrote a valid one that fits the surface's memory.
    pub fn check(&self) -> Option<(Kind, usize, usize)> {
        if self.len < HEADER || self.u32(O_MAGIC).load(Ordering::Acquire) != MAGIC { return None; }
        let kind = self.kind()?;
        let (width, height) = self.pair(O_SIZE);
        (width > 0 && height > 0 && bytes(kind, width, height) <= self.len).then_some((kind, width, height))
    }
    fn kind(&self) -> Option<Kind> {
        match self.u32(O_KIND).load(Ordering::Acquire) { 1 => Some(Kind::Text), 2 => Some(Kind::Pixels), _ => None }
    }
    /// The program's title (invalid UTF-8 is cut at the first bad byte).
    pub fn title(&self, out: &mut [u8; TITLE]) -> usize {
        unsafe { core::ptr::copy_nonoverlapping(self.base.add(O_TITLE), out.as_mut_ptr(), TITLE); }
        let len = out.iter().position(|&b| b == 0).unwrap_or(TITLE);
        match core::str::from_utf8(&out[..len]) { Ok(_) => len, Err(e) => e.valid_up_to() }
    }
    pub fn changes(&self) -> u32 { self.u32(O_CHANGES).load(Ordering::Acquire) }
    /// What the last changes touched, clipped to the size (None: everything).
    pub fn damage(&self) -> Option<(usize, usize, usize, usize)> {
        let ((x, y), (w, h)) = (self.pair(O_DAMAGE), self.pair(O_DAMAGE + 4));
        let (_, width, height) = self.check()?;
        (w > 0 && h > 0 && x < width && y < height).then(|| (x, y, w.min(width - x), h.min(height - y)))
    }
    pub fn ask_size(&self, width: usize, height: usize) { self.set_pair(O_WANTED, width.max(1), height.max(1)); }
    pub fn state(&self) -> u32 { self.u32(O_STATE).load(Ordering::Acquire) }
    pub fn set_state(&self, state: u32) { self.u32(O_STATE).store(state, Ordering::Release); }
    /// Queues an input event for the program; false while its queue is full.
    pub fn push_event(&self, word: usize) -> bool {
        let head = self.u32(O_EVENT_HEAD).load(Ordering::Relaxed);
        let tail = self.u32(O_EVENT_TAIL).load(Ordering::Acquire);
        if head.wrapping_sub(tail) as usize >= EVENTS { return false; }
        self.u64(O_EVENTS + 8 * (head as usize % EVENTS)).store(word as u64, Ordering::Relaxed);
        self.u32(O_EVENT_HEAD).store(head.wrapping_add(1), Ordering::Release);
        true
    }
    /// Cell (x, y) of a text surface the header describes: (character, foreground, background).
    pub fn cell(&self, x: usize, y: usize) -> Option<(char, u32, u32)> {
        let (Kind::Text, width, height) = self.check()? else { return None };
        if x >= width || y >= height { return None; }
        let at = HEADER + (y * width + x) * CELL;
        let word = |i: usize| unsafe { core::ptr::read_volatile(self.base.add(at + 4 * i).cast::<u32>()) };
        Some((char::from_u32(word(0)).unwrap_or('?'), word(1), word(2)))
    }
    /// Writes cell (x, y) of a text surface (program side).
    pub fn set_cell(&self, x: usize, y: usize, ch: char, fg: u32, bg: u32) {
        let (width, height) = self.pair(O_SIZE);
        if x >= width || y >= height || HEADER + (y * width + x + 1) * CELL > self.len { return; }
        let at = HEADER + (y * width + x) * CELL;
        for (i, value) in [ch as u32, fg, bg].into_iter().enumerate() { unsafe { core::ptr::write_volatile(self.base.add(at + 4 * i).cast::<u32>(), value); } }
    }
}
