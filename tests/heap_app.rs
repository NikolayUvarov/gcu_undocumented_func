//! Test-only heap client/adversary, substituted for app2 in an isolated VM.
#![no_std]
#![no_main]
use core::arch::asm;
#[path = "../common/abi.rs"]
mod abi;
use abi::{SyscallMailbox, HEAP_MAX_BLOCKS, HEAP_MAX_BYTES, HEAP_PAGE_SIZE as PAGE};

unsafe fn call(mb: *mut SyscallMailbox, number: usize, a: usize, b: usize) -> usize {
    (*mb).syscall_num = number;
    (*mb).arg1 = a;
    (*mb).arg2 = b;
    asm!("int 0x80");
    (*mb).result
}
unsafe fn print(mb: *mut SyscallMailbox, text: &[u8]) {
    call(mb, 3, text.as_ptr() as usize, text.len());
}
unsafe fn allocate(mb: *mut SyscallMailbox, size: usize) -> usize {
    call(mb, abi::SYSCALL_ALLOC, size, 0)
}
unsafe fn free(mb: *mut SyscallMailbox, address: usize) {
    assert_eq!(call(mb, abi::SYSCALL_FREE, address, 0), 0);
}
unsafe fn verify_zero(pointer: usize, size: usize) {
    for i in 0..size {
        assert_eq!((pointer as *const u8).add(i).read_volatile(), 0);
    }
}
unsafe fn checks(mb: *mut SyscallMailbox) {
    for size in [0, usize::MAX, usize::MAX - PAGE, HEAP_MAX_BYTES + 1] {
        assert_eq!(allocate(mb, size), 0);
    }
    let p = allocate(mb, PAGE + 1);
    assert_ne!(p, 0);
    assert_eq!(p % PAGE, 0);
    verify_zero(p, PAGE * 2);
    for i in 0..PAGE * 2 {
        (p as *mut u8).add(i).write_volatile(0xa5);
    }
    let q = allocate(mb, PAGE);
    assert_eq!(q, p + PAGE * 3);
    for bad in [0, p + 1, p + PAGE, p + PAGE * 2, 0x100000, usize::MAX] {
        assert_eq!(call(mb, abi::SYSCALL_FREE, bad, 0), usize::MAX);
    }
    free(mb, p);
    assert_eq!(call(mb, 3, p, 1), usize::MAX);
    assert_eq!(call(mb, abi::SYSCALL_FREE, p, 0), usize::MAX);
    assert_eq!(allocate(mb, PAGE + 1), p);
    verify_zero(p, PAGE * 2);
    assert_eq!(call(mb, 3, p + PAGE * 2 - 1, 2), usize::MAX);
    free(mb, q);
    free(mb, p);
    let mut blocks = [0; HEAP_MAX_BLOCKS];
    for block in &mut blocks {
        *block = allocate(mb, 1);
        assert_ne!(*block, 0);
    }
    assert_eq!(allocate(mb, 1), 0);
    for block in blocks {
        free(mb, block);
    }
    let full = allocate(mb, HEAP_MAX_BYTES);
    assert_ne!(full, 0);
    (full as *mut u8).write_volatile(42);
    ((full + HEAP_MAX_BYTES - 1) as *mut u8).write_volatile(43);
    assert_eq!(allocate(mb, 1), 0);
    free(mb, full);
    // Repeatedly add/remove PTs across a 2 MiB boundary. The harness compares
    // kernel heap usage before/after while this process remains alive.
    for _ in 0..20 {
        let p = allocate(mb, 0x202000);
        assert_ne!(p, 0);
        for offset in (0..0x202000).step_by(PAGE) {
            assert_eq!(((p + offset) as *const u8).read_volatile(), 0);
            ((p + offset) as *mut u8).write_volatile(0xa5);
        }
        free(mb, p);
    }
    print(mb, b"HEAP CHECKS OK\r\n");
}

unsafe fn stress(mb: *mut SyscallMailbox) {
    let p = allocate(mb, PAGE * 8);
    assert_ne!(p, 0);
    let marker = call(mb, 1, 0, 0);
    for i in 0..PAGE {
        (p as *mut usize).add(i).write_volatile(marker ^ i);
    }
    print(mb, b"HEAP STRESS READY\r\n");
    let mut iteration = 0;
    loop {
        for i in 0..PAGE {
            assert_eq!((p as *const usize).add(i).read_volatile(), marker ^ i);
        }
        let bytes = (iteration % 16 + 1) * PAGE;
        let scratch = allocate(mb, bytes);
        assert_ne!(scratch, 0);
        for i in (0..bytes).step_by(PAGE) {
            assert_eq!(((scratch + i) as *const u8).read_volatile(), 0);
            ((scratch + i) as *mut u8).write_volatile(0xff);
        }
        free(mb, scratch);
        iteration += 1;
        if call(mb, 2, 0, 0) as u8 == b'e' {
            return;
        } // kernel reclaims p
        call(mb, abi::SYSCALL_WAIT, 10, 0);
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_: &abi::BootInfo, mb: *mut SyscallMailbox) {
    unsafe {
        print(mb, b"HEAP READY\r\n");
        let mut held = 0;
        loop {
            match call(mb, 2, 0, 0) as u8 {
                b't' => checks(mb),
                b'c' => {
                    stress(mb);
                    return;
                }
                b'b' => {
                    assert_eq!(held, 0);
                    held = allocate(mb, HEAP_MAX_BYTES);
                    if held == 0 {
                        print(mb, b"HEAP OOM\r\n");
                    } else {
                        print(mb, b"HEAP QUOTA HELD\r\n");
                    }
                    print(mb, b"HEAP ALLOCATION FINISHED\r\n");
                }
                b'e' => {
                    assert_ne!(allocate(mb, PAGE * 3), 0);
                    return; // deliberate leak; process teardown must reclaim it
                }
                b'n' => {
                    let p = allocate(mb, PAGE);
                    assert_ne!(p, 0);
                    (p as *mut u8).write_volatile(0xc3);
                    asm!("call {target}", target = in(reg) p);
                    panic!("heap executed");
                }
                b'g' => {
                    let p = allocate(mb, PAGE + 1);
                    assert_ne!(p, 0);
                    ((p + PAGE * 2) as *mut u8).write_volatile(42);
                    panic!("guard writable");
                }
                b'u' => {
                    let p = allocate(mb, PAGE);
                    assert_ne!(p, 0);
                    (p as *mut u8).write_volatile(42); // populate TLB
                    free(mb, p);
                    (p as *mut u8).write_volatile(43);
                    panic!("freed page writable");
                }
                _ => {
                    call(mb, abi::SYSCALL_WAIT, 10, 0);
                }
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn memset(
    dest: *mut core::ffi::c_void,
    value: i32,
    len: usize,
) -> *mut core::ffi::c_void {
    for i in 0..len {
        dest.cast::<u8>().add(i).write_volatile(value as u8);
    }
    dest
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe {
        asm!("ud2", options(noreturn));
    }
}
