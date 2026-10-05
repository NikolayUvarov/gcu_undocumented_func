// The boot and diagnostic serial line: the PL011 UART of `virt` (UEFI has set it up).
const PL011: usize = 0x0900_0000;

pub unsafe fn init_serial() {}
pub unsafe fn serial_write_byte(b: u8) {
    while core::ptr::read_volatile((PL011 + 0x18) as *const u32) & 0x20 != 0 {} // TXFF
    core::ptr::write_volatile(PL011 as *mut u32, b as u32);
}
