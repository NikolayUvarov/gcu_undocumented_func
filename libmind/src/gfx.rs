//! Drawing into the process's own screen (the compositor copies it into the GOP framebuffer).
use crate::abi::BootInfo;
use crate::font::FONT;

#[derive(Clone, Copy)]
pub struct Screen { fb: *mut u32, pub width: usize, pub height: usize, pub stride: usize }

impl Screen {
    /// None for services: they are not given a screen.
    pub fn new(info: &BootInfo) -> Option<Self> {
        (!info.fb_ptr.is_null()).then_some(Self { fb: info.fb_ptr, width: info.width, height: info.height, stride: info.stride })
    }
    /// Pixels at `fb`, `stride` apart from row to row (a pixel window's surface).
    /// # Safety
    /// `fb` must map `stride * height` writable pixels for as long as the screen is used.
    pub unsafe fn at(fb: *mut u32, width: usize, height: usize, stride: usize) -> Self { Self { fb, width, height, stride } }
    pub fn pixel(&self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height { unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), color) } }
    }
    pub fn get(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height { unsafe { core::ptr::read_volatile(self.fb.add(y * self.stride + x)) } } else { 0 }
    }
    pub fn fill(&self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        for row in y..(y + height).min(self.height) { for column in x..(x + width).min(self.width) { unsafe { core::ptr::write_volatile(self.fb.add(row * self.stride + column), color) } } }
    }
    pub fn clear(&self, color: u32) { self.fill(0, 0, self.width, self.height, color) }
    /// One character of the 8x16 font (MIND Mono 16) at a pixel position; `background` fills the unset pixels.
    pub fn glyph16(&self, x: usize, y: usize, ch: char, color: u32, background: Option<u32>) {
        let rows = crate::font16::glyph(ch);
        for (row, &bits) in rows.iter().enumerate() {
            if y + row >= self.height { break; }
            for col in 0..8 {
                if x + col >= self.width { break; }
                let lit = bits & (0x80 >> col) != 0;
                let Some(pixel) = (if lit { Some(color) } else { background }) else { continue };
                unsafe { core::ptr::write_volatile(self.fb.add((y + row) * self.stride + x + col), pixel) }
            }
        }
    }
    /// One character of the 8x16 font stretched over a cell of `width` × `height` pixels (wm's scale, 000-APP-0061):
    /// each pixel takes the font's pixel under it; at 8 × 16 it is `glyph16`.
    #[allow(clippy::too_many_arguments)]
    pub fn glyph_cell(&self, x: usize, y: usize, width: usize, height: usize, ch: char, color: u32, background: Option<u32>) {
        if (width, height) == (8, 16) { return self.glyph16(x, y, ch, color, background); }
        let rows = crate::font16::glyph(ch);
        for py in 0..height.min(self.height.saturating_sub(y)) {
            let bits = rows[py * 16 / height];
            for px in 0..width.min(self.width.saturating_sub(x)) {
                let lit = bits & (0x80 >> (px * 8 / width)) != 0;
                let Some(pixel) = (if lit { Some(color) } else { background }) else { continue };
                unsafe { core::ptr::write_volatile(self.fb.add((y + py) * self.stride + x + px), pixel) }
            }
        }
    }
    /// UTF-8 text in the 8x16 font, one 8-pixel cell per character; returns the number of characters drawn.
    pub fn text16(&self, x: usize, y: usize, text: &str, color: u32, background: Option<u32>) -> usize {
        let mut count = 0;
        for ch in text.chars() {
            let at = x + count * 8;
            if at >= self.width { break; }
            self.glyph16(at, y, ch, color, background);
            count += 1;
        }
        count
    }
    /// Text in an 8x8 font (lowercase is drawn as uppercase); `background` fills the glyph background.
    pub fn text(&self, x: usize, y: usize, text: &[u8], scale: usize, color: u32, background: Option<u32>) {
        for (index, &ch) in text.iter().enumerate() {
            let code = ch.to_ascii_uppercase();
            let glyph = FONT[if (32..96).contains(&code) { (code - 32) as usize } else { 0 }];
            for row in 0..8 { for col in 0..8 {
                let lit = glyph & (1 << ((7 - row) * 8 + 7 - col)) != 0;
                let Some(pixel) = (if lit { Some(color) } else { background }) else { continue };
                self.fill(x + (index * 8 + col) * scale, y + row * scale, scale, scale, pixel);
            } }
        }
    }
}
