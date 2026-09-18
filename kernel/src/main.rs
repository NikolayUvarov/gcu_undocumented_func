#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ffi::c_void;

#[repr(C)]
pub struct BootInfo {
    pub fb_ptr: *mut u8,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub app_ptr: *const u8,
}

unsafe fn outb(port: u16, val: u8) { asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack)); }
unsafe fn inb(port: u16) -> u8 {
    let mut val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

const COM1: u16 = 0x3F8;

unsafe fn init_serial() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x03);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}

unsafe fn serial_is_transmit_empty() -> bool { (inb(COM1 + 5) & 0x20) != 0 }
unsafe fn serial_has_data() -> bool { (inb(COM1 + 5) & 1) != 0 }

unsafe fn serial_write_byte(b: u8) {
    while !serial_is_transmit_empty() {}
    outb(COM1, b);
}

unsafe fn serial_read_byte() -> u8 {
    if serial_has_data() { inb(COM1) } else { 0 }
}

const FONT: [u64; 64] = [
    0x0000000000000000, 0x1818181818001800, 0x6C6C000000000000, 0x36367F367F363600,
    0x183E603C067C1800, 0x60660C1830660600, 0x386C6C386CA6CC78, 0x1818300000000000,
    0x0C18303030180C00, 0x30180C0C0C183000, 0x00663CFF3C660000, 0x0018187E18180000,
    0x0000000000181830, 0x0000007E00000000, 0x0000000000181800, 0x060C183060C08000,
    0x3C666E7666663C00, 0x1838181818187E00, 0x3C66061C30607E00, 0x3C66061C06663C00,
    0x1C3C6CccFE0C0C00, 0x7E607C0606663C00, 0x3C607C6666663C00, 0x7E060C1830303000,
    0x3C66663C66663C00, 0x3C66663E06063C00, 0x0018180000181800, 0x0018180000181830,
    0x060C1830180C0600, 0x00007E007E000000, 0x30180C060C183000, 0x3C66060C18001800,
    0x3C666E6E60663C00, 0x183C66667E666600, 0x7C66667C66667C00, 0x3C66606060663C00,
    0x786C6666666C7800, 0x7E60607C60607E00, 0x7E60607C60606000, 0x3C66606E66663E00,
    0x6666667E66666600, 0x3E18181818183E00, 0x0606060606663C00, 0x666C7870786C6600,
    0x6060606060607E00, 0xC6EEDBc6c6c6c600, 0x66767E7E6E666600, 0x3C66666666663C00,
    0x7C66667C60606000, 0x3C6666666E3C0200, 0x7C66667C6C666600, 0x3C66603C06663C00,
    0x7E18181818181800, 0x6666666666663C00, 0x66666666663C1800, 0xC6C6C6D6FEEEC600,
    0x66663C183C666600, 0x6666663C18181800, 0x7E060C1830607E00, 0x3C30303030303C00,
    0x6030180C06030100, 0x3C0C0C0C0C0C3C00, 0x183C660000000000, 0x00000000000000FF 
];

const SCANCODE_TO_ASCII: &[u8] = b"??1234567890-=?\tqwertyuiop[]\n?asdfghjkl;'`?\\zxcvbnm,./?*? ?";

struct Console {
    fb: *mut u32, width: usize, height: usize, stride: usize,
    cx: usize, cy: usize, bg_color: u32, fg_color: u32,
}

impl Console {
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
        unsafe {
            if ch == b'\n' { serial_write_byte(b'\r'); }
            serial_write_byte(ch);
        }

        if ch == b'\n' {
            self.cx = 0;
            self.cy += 10;
        } else if ch == 0x08 { 
            if self.cx >= 8 { self.cx -= 8; }
            for row in 0..8 {
                for col in 0..8 {
                    unsafe { core::ptr::write_volatile(self.fb.add((self.cy + row) * self.stride + (self.cx + col)), self.bg_color); }
                }
            }
        } else {
            let idx = if ch >= 32 && ch <= 95 { (ch - 32) as usize } 
                      else if ch >= 97 && ch <= 122 { (ch - 97 + 33) as usize } 
                      else { 0 };
            let bitmap = FONT[idx];
            for row in 0..8 {
                let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF;
                for col in 0..8 {
                    let color = if (row_data & (1 << (7 - col))) != 0 { self.fg_color } else { self.bg_color };
                    unsafe { core::ptr::write_volatile(self.fb.add((self.cy + row) * self.stride + (self.cx + col)), color); }
                }
            }
            self.cx += 8;
        }

        if self.cx >= self.width { self.cx = 0; self.cy += 10; }
        if self.cy >= self.height - 10 { self.scroll(); }
    }

    fn print(&mut self, s: &str) { for b in s.bytes() { self.print_char(b); } }
    
    fn clear(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color); }
            }
        }
        self.cx = 0; self.cy = 0;
    }
}

fn streq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    for i in 0..a.len() { if a[i] != b[i] { return false; } }
    true
}

// ПРАВИЛЬНЫЕ СИГНАТУРЫ C ABI ДЛЯ ВСТРОЕННЫХ ФУНКЦИЙ LLVM
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let s_u8 = s as *mut u8;
    for i in 0..n { core::ptr::write_volatile(s_u8.add(i), c as u8); }
    s
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d_u8 = dest as *mut u8;
    let s_u8 = src as *const u8;
    for i in 0..n { core::ptr::write_volatile(d_u8.add(i), core::ptr::read_volatile(s_u8.add(i))); }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    let s1_u8 = s1 as *const u8;
    let s2_u8 = s2 as *const u8;
    for i in 0..n {
        let a = core::ptr::read_volatile(s1_u8.add(i));
        let b = core::ptr::read_volatile(s2_u8.add(i));
        if a != b { return (a as i32) - (b as i32); }
    }
    0
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    unsafe { init_serial(); }

    let mut term = Console {
        fb: info.fb_ptr as *mut u32, width: info.width, height: info.height, stride: info.stride, 
        cx: 0, cy: 0,
        bg_color: 0x001E1E2E, fg_color: 0x00A6E3A1,
    };

    term.clear();
    term.print("MIND CORE EXTENDED WITH UART COM1.\n");
    term.print("ABI SYNC RESTORED.\n");
    term.print("AWAITING INPUT FROM HOST SERIAL OR PS/2.\n\nMIND> ");

    let mut input_buf = [0u8; 128];
    let mut input_len = 0;
    let mut last_scancode = 0;

    loop {
        let mut ascii_input: u8 = 0;

        let serial_byte = unsafe { serial_read_byte() };
        if serial_byte != 0 {
            if serial_byte == 0x0D { ascii_input = b'\n'; } 
            else if serial_byte == 0x7F { ascii_input = 0x08; } 
            else { ascii_input = serial_byte; }
        } else {
            let mut scancode: u8 = 0;
            unsafe {
                let mut status: u8;
                asm!("in al, 0x64", out("al") status);
                if (status & 1) == 1 { asm!("in al, 0x60", out("al") scancode); }
            }
            if scancode != 0 && scancode != last_scancode && scancode < 0x80 {
                last_scancode = scancode;
                if scancode == 0x0E { ascii_input = 0x08; } 
                else if scancode == 0x39 { ascii_input = b' '; } 
                else if (scancode as usize) < SCANCODE_TO_ASCII.len() {
                    ascii_input = SCANCODE_TO_ASCII[scancode as usize];
                }
            } else if scancode >= 0x80 {
                last_scancode = scancode; 
            }
        }

        if ascii_input != 0 && ascii_input != b'?' {
            if ascii_input == 0x08 { 
                if input_len > 0 {
                    input_len -= 1;
                    term.print_char(0x08);
                }
            } else if ascii_input == b'\n' { 
                term.print("\n");
                let cmd = &input_buf[0..input_len];
                
                if input_len > 0 {
                    if streq(cmd, b"help") {
                        term.print("- help  : INFO\n- clear : CLEAR SCREEN\n- ping  : PONG\n- boot  : LAUNCH USERSPACE\n");
                    } else if streq(cmd, b"clear") {
                        term.clear();
                    } else if streq(cmd, b"ping") {
                        term.print("PONG. HOST SERIAL LINK OPERATIONAL.\n");
                    } else if streq(cmd, b"boot") {
                        term.print("TRANSFERRING CONTROL TO USERSPACE...\n");
                        for _ in 0..10_000_000 { unsafe { asm!("nop"); } }
                        let app_entry: extern "sysv64" fn(&BootInfo) -> ! = unsafe { core::mem::transmute(info.app_ptr) };
                        app_entry(info);
                    } else {
                        term.print("UNKNOWN: ");
                        for i in 0..input_len { term.print_char(cmd[i]); }
                        term.print("\n");
                    }
                }
                input_len = 0;
                term.print("\nMIND> ");
            } else { 
                if input_len < input_buf.len() {
                    input_buf[input_len] = ascii_input;
                    input_len += 1;
                    term.print_char(ascii_input);
                }
            }
        }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
