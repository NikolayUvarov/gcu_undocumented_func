// The differential program for the emulator (215-PRT-0016), built for aarch64-unknown-none-softfloat: it runs `work`
// and hands its results to the test by `svc #0`, the buffer's address in x0 and its length in x1.
#![no_std]
#![no_main]

#[path = "work.rs"]
mod work;

static mut OUT: [u64; work::SLOTS] = [0; work::SLOTS];

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    let out = unsafe { &mut *core::ptr::addr_of_mut!(OUT) };
    work::work(out);
    unsafe { core::arch::asm!("svc #0", in("x0") out.as_ptr(), in("x1") work::SLOTS, options(nostack)) };
    loop {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { core::arch::asm!("brk #1", options(nostack, noreturn)) }
}
