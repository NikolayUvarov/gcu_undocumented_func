//! Рисование в собственный экран процесса (композитор переносит его в кадр GOP).
use crate::abi::BootInfo;
use crate::font::FONT;

#[derive(Clone, Copy)]
pub struct Screen { fb: *mut u32, pub width: usize, pub height: usize, pub stride: usize }

impl Screen {
    /// None у сервисов: экран им не выделяется.
    pub fn new(info: &BootInfo) -> Option<Self> {
        (!info.fb_ptr.is_null()).then_some(Self { fb: info.fb_ptr, width: info.width, height: info.height, stride: info.stride })
    }
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
    /// Текст шрифтом 8x8 (строчные выводятся заглавными); `background` закрашивает фон глифа.
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
