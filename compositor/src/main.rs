#![no_std]
#![no_main]
use core::panic::PanicInfo;
use core::arch::asm;
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, SyscallMailbox, SYSCALL_MEM_MAP, SYSCALL_ALLOC, SYSCALL_FREE, SYSCALL_CAP_DROP, SYSCALL_WAIT, SYSCALL_COMPOSITOR_PULL};

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mb_ptr: *mut SyscallMailbox) -> ! {
    let fb_bytes = info.stride * info.height * 4;

    unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_MEM_MAP); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 2); asm!("int 0x80", options(nostack)); }
    let gop_vaddr = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) } as *mut u32;

    unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_ALLOC); core::ptr::write_volatile(&mut (*mb_ptr).arg1, fb_bytes); asm!("int 0x80", options(nostack)); }
    let shadow_vaddr = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) } as *mut u32;

    loop {
        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_COMPOSITOR_PULL); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 3); asm!("int 0x80", options(nostack)); }
        let is_dirty = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) };

        if is_dirty == 1 {
            unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_MEM_MAP); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 3); asm!("int 0x80", options(nostack)); }
            let source_vaddr = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) } as *const u32;

            let pixels = info.stride * info.height;
            for i in 0..pixels {
                let px = unsafe { core::ptr::read_volatile(source_vaddr.add(i)) };
                if px != unsafe { core::ptr::read(shadow_vaddr.add(i)) } {
                    unsafe { core::ptr::write_volatile(gop_vaddr.add(i), px); }
                    unsafe { core::ptr::write(shadow_vaddr.add(i), px); }
                }
            }

            unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_FREE); core::ptr::write_volatile(&mut (*mb_ptr).arg1, source_vaddr as usize); asm!("int 0x80", options(nostack)); }
            unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_CAP_DROP); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 3); asm!("int 0x80", options(nostack)); }
        }

        unsafe { core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_WAIT); core::ptr::write_volatile(&mut (*mb_ptr).arg1, 15); asm!("int 0x80", options(nostack)); }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
