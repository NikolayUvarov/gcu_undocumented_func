//! The kernel's console on a 16550 in memory (kernel/src/arch/aarch64/serial.rs, 100000-PRT-0010) against a register
//! file: register n at n << shift, read and written width bytes wide, the bytes sent and taken, the room to send, and
//! DesignWare's busy-detect (IIR 7) cleared by reading USR, register 31. USR lies on a page that cannot be read, so a
//! read of it shows as the fault that ends a child process.
#[allow(dead_code)]
mod board {
    use core::sync::atomic::{AtomicUsize, Ordering};
    pub static UART: AtomicUsize = AtomicUsize::new(0);
    pub static UART_KIND: AtomicUsize = AtomicUsize::new(UART_16550);
    pub const UART_PL011: usize = 0;
    pub const UART_16550: usize = 1;
    pub static UART_SHIFT: AtomicUsize = AtomicUsize::new(0);
    pub static UART_WIDTH: AtomicUsize = AtomicUsize::new(1);
    pub fn get(value: &AtomicUsize) -> usize { value.load(Ordering::Relaxed) }
    pub fn set(value: &AtomicUsize, to: usize) { value.store(to, Ordering::Relaxed) }
}
#[path = "../kernel/src/arch/aarch64/serial.rs"]
#[allow(dead_code)]
mod serial;

extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn mprotect(addr: *mut u8, len: usize, prot: i32) -> i32;
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn _exit(code: i32) -> !;
}
const PAGE: usize = 4096;
const SIGSEGV: i32 = 11;

// Two pages, the second unreadable; the base puts registers 0 to 30 at the end of the first and USR on the second.
struct Registers { page: *mut u8, base: usize, shift: usize, width: usize }

impl Registers {
    fn new(shift: usize, width: usize) -> Registers {
        let page = unsafe { mmap(core::ptr::null_mut(), 2 * PAGE, 3, 0x22, -1, 0) };
        assert!(page as isize != -1, "mmap");
        assert_eq!(unsafe { mprotect(page.add(PAGE), PAGE, 0) }, 0, "mprotect");
        unsafe { core::ptr::write_bytes(page, 0xEE, PAGE) };
        let base = page as usize + PAGE - (31 << shift);
        board::set(&board::UART, base);
        board::set(&board::UART_SHIFT, shift);
        board::set(&board::UART_WIDTH, width);
        Registers { page, base, shift, width }
    }
    // Register n as the UART would hold it, the bytes beyond the width left alone.
    fn put(&self, n: usize, value: u8) {
        let at = (self.base + (n << self.shift)) as *mut u8;
        unsafe { at.write(value); for i in 1..self.width { at.add(i).write(0); } }
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        let at = (self.base + (n << self.shift)) as *const u8;
        (0..self.width).map(|i| unsafe { at.add(i).read() }).collect()
    }
    fn first_page(&self) -> &[u8] { unsafe { core::slice::from_raw_parts(self.page, PAGE) } }
}

// Runs `f` in a child and returns the signal that ended it, or None when it exited normally.
fn signal_of(f: impl FnOnce()) -> Option<i32> {
    let pid = unsafe { fork() };
    assert!(pid >= 0, "fork");
    if pid == 0 { f(); unsafe { _exit(0) } }
    let mut status = 0;
    assert_eq!(unsafe { waitpid(pid, &mut status, 0) }, pid);
    (status & 0x7F != 0).then_some(status & 0x7F)
}

#[test]
fn a_16550_in_memory_by_shift_and_width() {
    // One test, as the board's values are statics: a byte-wide 16550, a DesignWare UART's layout, a 16-bit one.
    for (shift, width) in [(0, 1), (2, 4), (1, 2)] {
        let r = Registers::new(shift, width);
        let what = format!("shift {} width {}", shift, width);
        // Sending: THRE (LSR bit 5) up, the byte lands in register 0, written width bytes wide.
        r.put(5, 0x20);
        unsafe { serial::serial_write_byte(b'M') };
        assert_eq!(r.bytes(0), [&[b'M'][..], &vec![0; width - 1][..]].concat(), "{}", what);
        assert!(unsafe { serial::console_room() }, "{}", what);
        r.put(5, 0x00);
        assert!(!unsafe { serial::console_room() }, "{}", what);
        // Taking: bytes while LSR's data ready (bit 0) is up, none once it is down.
        r.put(0, b'k');
        r.put(5, 0x21);
        let mut out = [0u8; 3];
        assert_eq!(unsafe { serial::console_take(&mut out) }, 3, "{}", what);
        assert_eq!(&out, b"kkk", "{}", what);
        r.put(5, 0x20);
        assert_eq!(unsafe { serial::console_take(&mut out) }, 0, "{}", what);
        // Interrupts: IER (register 1) bits 0 and 1.
        unsafe { serial::console_interrupts(true, true) };
        assert_eq!(r.bytes(1)[0], 3, "{}", what);
        unsafe { serial::console_interrupts(true, false) };
        assert_eq!(r.bytes(1)[0], 1, "{}", what);
        // No other register was written: what is not one of 0, 1, 2 and 5 keeps its filler.
        let written: Vec<usize> = [0, 1, 2, 5].iter().flat_map(|&n| { let at = r.base - r.page as usize + (n << shift); at..at + width }).collect();
        assert!(r.first_page().iter().enumerate().all(|(i, &b)| written.contains(&i) || b == 0xEE), "{}", what);
        // IIR's other answers do not read USR: none, received data, the timeout, THRE, a line status, a modem status.
        for iir in [0xC1u8, 0xC4, 0xCC, 0xC2, 0xC6, 0x00, 0x01] {
            r.put(2, iir);
            assert_eq!(unsafe { serial::console_take(&mut out) }, 0, "{} IIR {:#x}", what, iir);
        }
        // Busy-detect, with the FIFOs on (0xC7) or off (0x07): USR is read, as the fault on its page shows.
        for iir in [0xC7u8, 0x07] {
            r.put(2, iir);
            assert_eq!(signal_of(|| { let mut out = [0u8; 1]; unsafe { serial::console_take(&mut out) }; }), Some(SIGSEGV), "{} IIR {:#x}", what, iir);
        }
    }
}
