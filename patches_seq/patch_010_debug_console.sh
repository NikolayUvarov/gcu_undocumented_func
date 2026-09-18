#!/bin/bash
set -e
ROOT_DIR=$(pwd)

echo ">>> Применяем Патч 010: Интерактивная Консоль Ядра (REPL)..."

cat << 'INNER_KERNEL' > kernel/src/main.rs
#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

// Минималистичный системный шрифт (ASCII 32..90). Каждый u64 - это 8 строк по 8 бит.
const FONT: [u64; 59] = [
    0x0000000000000000, 0x1818181818001800, 0x6C6C000000000000, 0x36367F367F363600, //   ! " #
    0x183E603C067C1800, 0x60660C1830660600, 0x386C6C386CA6CC78, 0x1818300000000000, // $ % & '
    0x0C18303030180C00, 0x30180C0C0C183000, 0x00663CFF3C660000, 0x0018187E18180000, // ( ) * +
    0x0000000000181830, 0x0000007E00000000, 0x0000000000181800, 0x060C183060C08000, // , - . /
    0x3C666E7666663C00, 0x1838181818187E00, 0x3C66061C30607E00, 0x3C66061C06663C00, // 0 1 2 3
    0x1C3C6CccFE0C0C00, 0x7E607C0606663C00, 0x3C607C6666663C00, 0x7E060C1830303000, // 4 5 6 7
    0x3C66663C66663C00, 0x3C66663E06063C00, 0x0018180000181800, 0x0018180000181830, // 8 9 : ;
    0x060C1830180C0600, 0x00007E007E000000, 0x30180C060C183000, 0x3C66060C18001800, // < = > ?
    0x3C666E6E60663C00, 0x183C66667E666600, 0x7C66667C66667C00, 0x3C66606060663C00, // @ A B C
    0x786C6666666C7800, 0x7E60607C60607E00, 0x7E60607C60606000, 0x3C66606E66663E00, // D E F G
    0x6666667E66666600, 0x3E18181818183E00, 0x0606060606663C00, 0x666C7870786C6600, // H I J K
    0x6060606060607E00, 0xC6EEDBc6c6c6c600, 0x66767E7E6E666600, 0x3C66666666663C00, // L M N O
    0x7C66667C60606000, 0x3C6666666E3C0200, 0x7C66667C6C666600, 0x3C66603C06663C00, // P Q R S
    0x7E18181818181800, 0x6666666666663C00, 0x66666666663C1800, 0xC6C6C6D6FEEEC600, // T U V W
    0x66663C183C666600, 0x6666663C18181800, 0x7E060C1830607E00, 0x3C30303030303C00, // X Y Z [
    0x6030180C06030100, 0x3C0C0C0C0C0C3C00, 0x183C660000000000, 0x00000000000000FF  // \ ] ^ _
];

// Английская раскладка PS/2 (Скан-коды 0x01..0x39)
const SCANCODE_TO_ASCII: &[u8] = b"??1234567890-=?\tqwertyuiop[]\n?asdfghjkl;'`?\\zxcvbnm,./?*? ?";

struct Console {
    fb: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
    cx: usize,
    cy: usize,
    bg_color: u32,
    fg_color: u32,
}

impl Console {
    fn draw_char(&self, x: usize, y: usize, ch: u8) {
        let idx = if ch >= 32 && ch <= 95 { (ch - 32) as usize } 
                  else if ch >= 97 && ch <= 122 { (ch - 97 + 33) as usize } // a-z -> A-Z
                  else { 0 };
        
        let bitmap = FONT[idx];
        for row in 0..8 {
            let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF;
            for col in 0..8 {
                let color = if (row_data & (1 << (7 - col))) != 0 { self.fg_color } else { self.bg_color };
                unsafe { core::ptr::write_volatile(self.fb.add((y + row) * self.stride + (x + col)), color); }
            }
        }
    }

    fn scroll(&mut self) {
        let line_h = 10;
        let limit = self.height - line_h;
        for y in line_h..limit {
            for x in 0..self.width {
                unsafe {
                    let pixel = core::ptr::read_volatile(self.fb.add(y * self.stride + x));
                    core::ptr::write_volatile(self.fb.add((y - line_h) * self.stride + x), pixel);
                }
            }
        }
        for y in (limit - line_h)..limit {
            for x in 0..self.width {
                unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color); }
            }
        }
        self.cy -= line_h;
    }

    fn print_char(&mut self, ch: u8) {
        if ch == b'\n' {
            self.cx = 0;
            self.cy += 10;
        } else if ch == 0x08 { // Backspace
            if self.cx >= 8 { self.cx -= 8; }
            self.draw_char(self.cx, self.cy, b' ');
        } else {
            self.draw_char(self.cx, self.cy, ch);
            self.cx += 8;
        }

        if self.cx >= self.width {
            self.cx = 0;
            self.cy += 10;
        }
        if self.cy >= self.height - 10 {
            self.scroll();
        }
    }

    fn print(&mut self, s: &str) {
        for b in s.bytes() { self.print_char(b); }
    }

    fn clear(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color); }
            }
        }
        self.cx = 0;
        self.cy = 0;
    }
}

// Сравнение строк (no_std)
fn streq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    for i in 0..a.len() { if a[i] != b[i] { return false; } }
    true
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(fb_ptr: *mut u32, bb_ptr: *mut u32, width: usize, height: usize, stride: usize, app_ptr: *const u8) -> ! {
    // Вся память живет в стеке! Никакого .bss
    let mut term = Console {
        fb: fb_ptr, width, height, stride,
        cx: 0, cy: 0,
        bg_color: 0x001E1E2E, // Терминальный фон (Catppuccin)
        fg_color: 0x00A6E3A1, // Зеленый текст
    };

    term.clear();
    term.print("MIND CORE INITIALIZED.\n");
    term.print("SUBSTRATE: X86_64 FLAT BINARY.\n");
    term.print("TYPE 'help' FOR COMMANDS.\n\nMIND> ");

    let mut input_buf = [0u8; 128];
    let mut input_len = 0;
    let mut last_scancode = 0;

    loop {
        let mut scancode: u8 = 0;
        unsafe {
            let mut status: u8;
            asm!("in al, 0x64", out("al") status);
            if (status & 1) == 1 { asm!("in al, 0x60", out("al") scancode); }
        }

        if scancode != 0 && scancode != last_scancode {
            last_scancode = scancode;
            
            // Обработка нажатия (отпускание клавиш > 0x80 игнорируем)
            if scancode < 0x80 {
                let mut ascii = if (scancode as usize) < SCANCODE_TO_ASCII.len() {
                    SCANCODE_TO_ASCII[scancode as usize]
                } else { b'?' };

                // Обработка Backspace (scancode 0x0E)
                if scancode == 0x0E {
                    if input_len > 0 {
                        input_len -= 1;
                        term.print_char(0x08); // Сигнал терминалу стереть символ
                    }
                } 
                // Обработка Пробела (scancode 0x39)
                else if scancode == 0x39 {
                    if input_len < input_buf.len() {
                        input_buf[input_len] = b' ';
                        input_len += 1;
                        term.print_char(b' ');
                    }
                }
                // Обработка Enter
                else if ascii == b'\n' {
                    term.print("\n");
                    let cmd = &input_buf[0..input_len];
                    
                    if input_len > 0 {
                        if streq(cmd, b"help") {
                            term.print("AVAILABLE COMMANDS:\n");
                            term.print("- help  : SHOW THIS MESSAGE\n");
                            term.print("- clear : CLEAR SCREEN\n");
                            term.print("- ping  : PONG\n");
                            term.print("- boot  : LAUNCH USERSPACE APPLICATION\n");
                        } else if streq(cmd, b"clear") {
                            term.clear();
                        } else if streq(cmd, b"ping") {
                            term.print("PONG. COMMUNICATION LINK ACTIVE.\n");
                        } else if streq(cmd, b"boot") {
                            term.print("TRANSFERRING CONTROL TO USERSPACE...\n");
                            // Ждем секунду
                            for _ in 0..10_000_000 { unsafe { asm!("nop"); } }
                            let app_entry: extern "sysv64" fn(*mut u32, *mut u32, usize, usize, usize, *const u8) -> ! = unsafe { core::mem::transmute(app_ptr) };
                            app_entry(fb_ptr, bb_ptr, width, height, stride, core::ptr::null());
                        } else {
                            term.print("UNKNOWN COMMAND: ");
                            for i in 0..input_len { term.print_char(cmd[i]); }
                            term.print("\n");
                        }
                    }
                    input_len = 0;
                    term.print("\nMIND> ");
                }
                // Обычные символы
                else if ascii != b'?' {
                    if input_len < input_buf.len() {
                        input_buf[input_len] = ascii;
                        input_len += 1;
                        term.print_char(ascii);
                    }
                }
            }
        }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
INNER_KERNEL

chmod +x 02_build.sh
./02_build.sh

echo "====================================================="
echo "Патч 010 применен. Собрано. Запускай QEMU."
echo "У нас есть рабочий шелл в Ring 0!"
echo "====================================================="
