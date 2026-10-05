// Port I/O.
use core::arch::asm;

pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}
pub unsafe fn outl(port: u16, val: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack));
}
pub unsafe fn inl(port: u16) -> u32 {
    let mut val: u32;
    asm!("in eax, dx", out("eax") val, in("dx") port, options(nomem, nostack));
    val
}
pub unsafe fn inb(port: u16) -> u8 {
    let mut val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

// A port read or write of 1, 2 or 4 bytes for a driver's port capability.
pub unsafe fn read(port: u16, width: usize) -> usize {
    match width { 1 => { let v: u8; asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack)); v as usize } 2 => { let v: u16; asm!("in ax, dx", out("ax") v, in("dx") port, options(nomem, nostack)); v as usize } _ => { let v: u32; asm!("in eax, dx", out("eax") v, in("dx") port, options(nomem, nostack)); v as usize } }
}
pub unsafe fn write(port: u16, width: usize, value: usize) {
    match width { 1 => asm!("out dx, al", in("dx") port, in("al") value as u8, options(nomem, nostack)), 2 => asm!("out dx, ax", in("dx") port, in("ax") value as u16, options(nomem, nostack)), _ => asm!("out dx, eax", in("dx") port, in("eax") value as u32, options(nomem, nostack)) }
}
