#![no_std]
#![no_main]
use core::panic::PanicInfo;
use core::arch::asm;
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, SyscallMailbox, SYSCALL_IPC_SEND, SYSCALL_IPC_RECV};

fn port_out(mb: *mut SyscallMailbox, cap: usize, port: u16, val: u8) {
    unsafe {
        core::ptr::write_volatile(&mut (*mb).syscall_num, 18); // SYSCALL_PORT_OUT
        core::ptr::write_volatile(&mut (*mb).arg1, cap);
        core::ptr::write_volatile(&mut (*mb).arg2, port as usize);
        core::ptr::write_volatile(&mut (*mb).msg[0], val as usize);
        asm!("int 0x80", options(nostack));
    }
}
fn port_in(mb: *mut SyscallMailbox, cap: usize, port: u16) -> u8 {
    unsafe {
        core::ptr::write_volatile(&mut (*mb).syscall_num, 17); // SYSCALL_PORT_IN
        core::ptr::write_volatile(&mut (*mb).arg1, cap);
        core::ptr::write_volatile(&mut (*mb).arg2, port as usize);
        asm!("int 0x80", options(nostack));
        core::ptr::read_volatile(&(*mb).result) as u8
    }
}

const SECONDS: u8 = 0x00; const MINUTES: u8 = 0x02; const HOURS: u8 = 0x04;
const STATUS_A: u8 = 0x0A; const STATUS_B: u8 = 0x0B; const UPDATE_IN_PROGRESS: u8 = 0x80;

fn read_time(mb: *mut SyscallMailbox) -> Option<usize> {
    let mut read = |reg: u8| -> u8 {
        port_out(mb, 2, 0x70, reg);
        port_in(mb, 3, 0x71)
    };
    for _ in 0..8 {
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 { continue; }
        let first = [read(SECONDS), read(MINUTES), read(HOURS), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 { continue; }
        let second = [read(SECONDS), read(MINUTES), read(HOURS), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS == 0 && first == second {
            return decode_time(second);
        }
    }
    None
}

fn decode_time([seconds, minutes, hours, mode]: [u8; 4]) -> Option<usize> {
    let decode = |value: u8| -> Option<u8> {
        if mode & 0x04 != 0 { Some(value) } else if value & 0x0F <= 9 && value >> 4 <= 9 { Some((value >> 4) * 10 + (value & 0x0F)) } else { None }
    };
    let second = decode(seconds)?; let minute = decode(minutes)?; let mut hour = decode(hours & 0x7F)?;
    if mode & 0x02 == 0 { if hour == 0 || hour > 12 { return None; } hour = hour % 12 + if hours & 0x80 != 0 { 12 } else { 0 }; } else if hours & 0x80 != 0 { return None; }
    if second >= 60 || minute >= 60 || hour >= 24 { return None; }
    Some(hour as usize * 3600 + minute as usize * 60 + second as usize)
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_info: &BootInfo, mb_ptr: *mut SyscallMailbox) -> () {
    let my_ep_slot = 1; // Захардкожено ядром для RTC
    loop {
        unsafe { 
            core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_IPC_RECV);
            core::ptr::write_volatile(&mut (*mb_ptr).arg1, my_ep_slot);
            core::ptr::write_volatile(&mut (*mb_ptr).arg2, 5); // Слот для Reply EP клиента
            asm!("int 0x80", options(nostack)); 
        }
        let res = unsafe { core::ptr::read_volatile(&(*mb_ptr).result) };
        if res != 0 { continue; }

        let time = read_time(mb_ptr).unwrap_or(usize::MAX);

        unsafe { 
            core::ptr::write_volatile(&mut (*mb_ptr).syscall_num, SYSCALL_IPC_SEND);
            core::ptr::write_volatile(&mut (*mb_ptr).arg1, 5);
            core::ptr::write_volatile(&mut (*mb_ptr).arg2, 0);
            core::ptr::write_volatile(&mut (*mb_ptr).msg[0], time);
            asm!("int 0x80", options(nostack)); 
        }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
