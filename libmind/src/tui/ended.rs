//! What a program in a window leaves on view when it ends with a failure (211-APP-0039): the last lines it printed
//! and its status, until a key. `Tail` keeps the last bytes a program printed; `draw` lays them out in a grid.
use super::{Grid, Rect, Theme};
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

/// The last `N` bytes pushed into it.
pub struct Tail<const N: usize> { busy: AtomicBool, kept: UnsafeCell<(usize, [u8; N])> }

// `kept` is touched only while `busy` is held.
unsafe impl<const N: usize> Sync for Tail<N> {}

impl<const N: usize> Tail<N> {
    pub const fn new() -> Self { Self { busy: AtomicBool::new(false), kept: UnsafeCell::new((0, [0; N])) } }

    pub fn push(&self, bytes: &[u8]) {
        let bytes = &bytes[bytes.len().saturating_sub(N)..];
        self.with(|len, kept| {
            let gone = (*len + bytes.len()).saturating_sub(N);
            kept.copy_within(gone..*len, 0);
            *len -= gone;
            kept[*len..*len + bytes.len()].copy_from_slice(bytes);
            *len += bytes.len();
        })
    }

    /// Copies what is kept into `out`; returns its length.
    pub fn copy(&self, out: &mut [u8; N]) -> usize {
        self.with(|len, kept| { out[..*len].copy_from_slice(&kept[..*len]); *len })
    }

    fn with<R>(&self, f: impl FnOnce(&mut usize, &mut [u8; N]) -> R) -> R {
        while self.busy.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); }
        let (len, kept) = unsafe { &mut *self.kept.get() };
        let result = f(len, kept);
        self.busy.store(false, Ordering::Release);
        result
    }
}

// A printed line as text: a character cut at the tail's start, a carriage return and invalid bytes left out.
fn line_text(bytes: &[u8]) -> &str {
    let start = bytes.iter().position(|&b| b & 0xC0 != 0x80).unwrap_or(bytes.len());
    let bytes = &bytes[start..];
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    match core::str::from_utf8(bytes) { Ok(text) => text, Err(error) => core::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or("") }
}

/// Lays out what a program that ended with `status` leaves in its window: the last lines of `printed` that fit, wrapped
/// at the grid's width, above the line `ENDED (STATUS n): PRESS A KEY`.
pub fn draw(grid: &mut Grid, printed: &[u8], status: u32, theme: &Theme) {
    let (cols, rows) = (grid.cols, grid.rows);
    if cols == 0 || rows == 0 { return; }
    grid.fill(Rect::new(0, 0, cols, rows), ' ', theme.panel);
    let trailing = printed.last() == Some(&b'\n');
    let lines = || printed.split(|&b| b == b'\n').map(line_text);
    let count = if printed.is_empty() { 0 } else { lines().count() - trailing as usize };
    let height = |line: &str| line.chars().count().max(1).div_ceil(cols);
    // From the last line back, as many whole lines as fit above the status line.
    let (mut first, mut used) = (count, 0);
    for line in lines().rev().skip(trailing as usize).take(count) {
        if used + height(line) > rows - 1 { break; }
        used += height(line);
        first -= 1;
    }
    let mut y = 0;
    for line in lines().skip(first).take(count - first) {
        let mut rest = line;
        loop {
            let cut = rest.char_indices().nth(cols).map_or(rest.len(), |(at, _)| at);
            grid.text(0, y, &rest[..cut], theme.panel);
            y += 1;
            rest = &rest[cut..];
            if rest.is_empty() { break; }
        }
    }
    let mut ended = crate::util::FixedBuf::<64>::new();
    let _ = write!(ended, "ENDED (STATUS {}): PRESS A KEY", status);
    grid.text_padded(0, rows - 1, ended.as_str(), cols, theme.selected);
}
