#![no_std]
#![no_main]
use core::{arch::asm, panic::PanicInfo};
#[path = "../../common/abi.rs"] mod abi;
mod cycle; mod face;
#[path = "../../common/font.rs"] mod font;
mod view;
use abi::{BootInfo, SyscallMailbox, SYSCALL_UPTIME, SYSCALL_ENDPOINT_CREATE, SYSCALL_IPC_SEND, SYSCALL_IPC_RECV, SYSCALL_WAIT};
use cycle::{point_at, Cycle, OrbitMode}; use face::{time_text, Face}; use view::View;

fn wait_and_check(mb: *mut SyscallMailbox, ms: usize) -> u8 {
    unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 2); asm!("int 0x80", options(nostack)); }
    let key = unsafe { core::ptr::read_volatile(&(*mb).result) } as u8;
    if key == 0x01 || key == 0x1B { unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, 7); asm!("int 0x80", options(nostack)); } }
    unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_WAIT); core::ptr::write_volatile(&mut (*mb).arg1, ms); asm!("int 0x80", options(nostack)); }
    key
}

fn get_rtc_time(mb: *mut SyscallMailbox, reply_ep_slot: usize) -> usize {
    loop {
        unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IPC_SEND); core::ptr::write_volatile(&mut (*mb).arg1, 2); core::ptr::write_volatile(&mut (*mb).arg2, 0); core::ptr::write_volatile(&mut (*mb).msg[0], reply_ep_slot); core::ptr::write_volatile(&mut (*mb).msg[1], abi::CAP_WRITE as usize); asm!("int 0x80", options(nostack)); }
        let res = unsafe { core::ptr::read_volatile(&(*mb).result) };
        if res == 0 { break; }
        wait_and_check(mb, 10);
    }
    unsafe { core::ptr::write_volatile(&mut (*mb).syscall_num, SYSCALL_IPC_RECV); core::ptr::write_volatile(&mut (*mb).arg1, reply_ep_slot); core::ptr::write_volatile(&mut (*mb).arg2, 0); asm!("int 0x80", options(nostack)); }
    unsafe { core::ptr::read_volatile(&(*mb).msg[0]) }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo, mailbox: *mut SyscallMailbox) {
    let view = View::new(info); view.clear(); view.hints(true); view.face(Face::DARK); view.digital(b"--:--:--", true);
    
    unsafe { core::ptr::write_volatile(&mut (*mailbox).syscall_num, SYSCALL_ENDPOINT_CREATE); asm!("int 0x80", options(nostack)); }
    let reply_ep_slot = unsafe { core::ptr::read_volatile(&(*mailbox).result) };

    let mut previous_face = Face::DARK; let mut previous_time = None; let mut show_digits = true; let mut show_hints = true; let mut orbit_mode = OrbitMode::Off; let mut previous_mode = OrbitMode::Off; let mut cycle = Cycle::new(); let mut previous_dot = None;
    loop {
        let key = wait_and_check(mailbox, 100);
        if key == 0x20 || key == b'd' || key == b'D' { show_digits = !show_digits; let text = previous_time.map(time_text).unwrap_or(*b"--:--:--"); view.digital(&text, show_digits); }
        if key == 0x23 || key == b'h' || key == b'H' { show_hints = !show_hints; view.hints(show_hints); }
        let requested = match key { 0x2E | 0x63 | 0x43 => Some(OrbitMode::Simple), 0x19 | 0x70 | 0x50 => Some(OrbitMode::Ticks), _ => None };
        if let Some(requested) = requested { orbit_mode = orbit_mode.toggle(requested); }
        
        let seconds = get_rtc_time(mailbox, reply_ep_slot);
        unsafe { core::ptr::write_volatile(&mut (*mailbox).syscall_num, SYSCALL_UPTIME); asm!("int 0x80", options(nostack)); }
        let now = unsafe { core::ptr::read_volatile(&(*mailbox).result) };
        cycle.observe(seconds, now);
        if let Some(current) = Face::at(seconds) {
            if current != previous_face { view.face(current); previous_face = current; }
            if previous_time != Some(seconds) && show_digits { view.digital(&time_text(seconds), true); }
            previous_time = Some(seconds);
        }
        let dot = if orbit_mode != OrbitMode::Off { cycle.phase(now).map(|phase| point_at(phase, view.half() * 3 / 4)) } else { None };
        if dot != previous_dot || orbit_mode != previous_mode { view.cycle(previous_dot, dot, previous_mode, orbit_mode); previous_dot = dot; previous_mode = orbit_mode; }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
