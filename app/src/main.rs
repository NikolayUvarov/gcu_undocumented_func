#![no_std]
#![no_main]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::util::Decimal;

fn os_print(msg: &[u8]) { mind::process::log(msg) }
fn get_os_ticks() -> usize { mind::time::rdtsc() as usize }
// The character of a key event (0x1B for Esc), or 0.
fn get_os_key() -> u8 { mind::input::read_key().and_then(|k| k.char()).filter(|c| c.is_ascii()).map_or(0, |c| c as u8) }

// The two threads' saved stack pointers, and the work the second one counts (000-APP-0087: atomics, no mutable statics).
static MAIN_SP: AtomicU64 = AtomicU64::new(0);
static THREAD_SP: AtomicU64 = AtomicU64::new(0);
static WORK_COUNTER: AtomicUsize = AtomicUsize::new(0);

// The second thread's stack: only that thread uses it, after init_thread wrote its first frame.
#[repr(align(16))]
struct ThreadStack(UnsafeCell<[u64; 1024]>);
unsafe impl Sync for ThreadStack {}
static THREAD_STACK: ThreadStack = ThreadStack(UnsafeCell::new([0; 1024]));

// Saves the callee-saved registers on the current stack, stores its pointer in `old_sp`, switches to `new_sp` and
// restores the registers saved there.
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
extern "sysv64" fn yield_task(_old_sp: *mut u64, _new_sp: u64) {
    core::arch::naked_asm!("push rbx", "push rbp", "push r12", "push r13", "push r14", "push r15", "mov [rdi], rsp", "mov rsp, rsi", "pop r15", "pop r14", "pop r13", "pop r12", "pop rbp", "pop rbx", "ret");
}
#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
extern "C" fn yield_task(_old_sp: *mut u64, _new_sp: u64) {
    core::arch::naked_asm!("sub sp, sp, #96", "stp x19, x20, [sp]", "stp x21, x22, [sp, #16]", "stp x23, x24, [sp, #32]",
        "stp x25, x26, [sp, #48]", "stp x27, x28, [sp, #64]", "stp x29, x30, [sp, #80]", "mov x9, sp", "str x9, [x0]", "mov sp, x1",
        "ldp x19, x20, [sp]", "ldp x21, x22, [sp, #16]", "ldp x23, x24, [sp, #32]", "ldp x25, x26, [sp, #48]",
        "ldp x27, x28, [sp, #64]", "ldp x29, x30, [sp, #80]", "add sp, sp, #96", "ret");
}

// Never returns: its first frame holds no return address to go back to.
fn background_task() -> ! {
    loop {
        for _ in 0..500 { WORK_COUNTER.fetch_add(1, Ordering::Relaxed); }
        yield_task(THREAD_SP.as_ptr(), MAIN_SP.load(Ordering::Relaxed));
    }
}

unsafe fn init_thread() {
    let stack_ptr = THREAD_STACK.0.get() as *mut u64;
    let mut sp = stack_ptr.add(1024) as u64;
    #[cfg(target_arch = "x86_64")] {
        // A SysV function starts with RSP % 16 == 8 after its return address.
        sp -= 8;
        sp -= 8; *(sp as *mut u64) = background_task as *const () as u64;
        for _ in 0..6 { sp -= 8; *(sp as *mut u64) = 0; }
    }
    #[cfg(target_arch = "aarch64")] {
        // The frame yield_task restores: x19-x28, x29 = 0, x30 = the entry; sp stays 16-byte aligned.
        sp -= 96;
        for i in 0..12 { *((sp + 8 * i) as *mut u64) = 0; }
        *((sp + 88) as *mut u64) = background_task as *const () as u64;
    }
    THREAD_SP.store(sp, Ordering::Relaxed);
}

const SIN_TABLE: [isize; 36] = [0, 17, 34, 50, 64, 76, 86, 93, 98, 100, 98, 93, 86, 76, 64, 50, 34, 17, 0, -17, -34, -50, -64, -76, -86, -93, -98, -100, -98, -93, -86, -76, -64, -50, -34, -17];

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("app — graphics demo: an animation on its own screen.\nUsage: app (or boot)\nCtrl+Z: back to the shell, the demo keeps running; Esc: exit.");
    // In wm a window of its own (000-APP-0056: otherwise it ran unseen, with no screen to draw on).
    let info = mind::windowed::pixels(info, 600, 240, "app");
    let Some(screen) = Screen::new(info) else { os_print(b"[APP] NO SCREEN TO DRAW ON. EXITING.\r\n"); return };
    unsafe { init_thread(); }

    os_print(b"\r\n========================================\r\n");
    os_print(b"[APP] STARTED. CTRL+Z: SHELL, ESC: EXIT.\r\n");
    os_print(b"========================================\r\n\r\n");

    let mut frame_counter: usize = 0;
    let (width, height) = (screen.width as isize, screen.height as isize);
    let (cx, cy) = (width / 2, height / 2);

    let mut color_theme: u32 = 0x0000FFFF; // Cyan by default
    let mut last_seen_key: u8 = 0;
    screen.clear(0x00111111);

    loop {
        yield_task(MAIN_SP.as_ptr(), THREAD_SP.load(Ordering::Relaxed));

        let os_key = get_os_key();
        if os_key != 0 {
            last_seen_key = os_key;
            match os_key {
                b' ' => color_theme = 0x00FF0000,
                b'b' => color_theme = 0x000000FF,
                b'g' => color_theme = 0x0000FF00,
                _ => {}
            }
            // Esc from the PS/2 keyboard or the UART
            if os_key == 0x1B {
                os_print(b"[SYSTEM] ESC PRESSED. EXITING APP...\r\n");
                break;
            }
        }

        // Erase only the square's drawing area and changing counters.
        let (left, top) = ((cx - 150).max(0) as usize, (cy - 150).max(0) as usize);
        screen.fill(left, top, ((cx + 150).min(width) as usize).saturating_sub(left), ((cy + 150).min(height) as usize).saturating_sub(top), 0x00111111);
        screen.fill(40, 40, 300, 128, 0x00111111);

        let size: isize = 120;
        let t_temp = frame_counter % 36;
        let sin_a = SIN_TABLE[t_temp];
        let cos_a = SIN_TABLE[(t_temp + 9) % 36];

        for y in (cy - 150).max(0)..(cy + 150).min(height) {
            for x in (cx - 150).max(0)..(cx + 150).min(width) {
                let dx = x - cx; let dy = y - cy;
                let rx = (dx * cos_a - dy * sin_a) / 100; let ry = (dx * sin_a + dy * cos_a) / 100;
                if rx > -size && rx < size && ry > -size && ry < size { screen.pixel(x as usize, y as usize, color_theme); }
            }
        }

        screen.text(40, 40, b"USERSPACE APP (ELF)", 1, color_theme, None);
        screen.text(40, 190, b"CTRL+Z: SHELL / ESC: EXIT", 1, 0x00FFFFFF, None);
        let rows: [(&[u8], usize, usize, usize); 4] = [(b"FRAMES:", 70, 120, frame_counter), (b"WORK  :", 100, 120, WORK_COUNTER.load(Ordering::Relaxed)),
            (b"OS TICKS:", 130, 140, get_os_ticks() / 10_000_000), (b"LAST KEY:", 160, 140, last_seen_key as usize)];
        for (label, y, x, value) in rows {
            screen.text(40, y, label, 1, 0x00FFFFFF, None);
            screen.text(x, y, Decimal::new(value).as_bytes(), 1, 0x00FFFF00, None);
        }

        frame_counter = frame_counter.wrapping_add(1);
        mind::time::sleep(30);
    }
}
