// The page's platform (215-PRT-0017): no I/O ports and no lines by number; the console's channel on line 1, and one
// node, the PL031 of the page's wall clock (`wasm_machine`).
pub fn ports(_start: usize, _end: usize) -> bool { false }
pub fn nodes() { unsafe { crate::platform_nodes::build(|table| { table.add(b"arm,pl031", 0, &[(wasm_machine::RTC, 0x1000)], &[wasm_machine::RTC_LINE]); }) } }
pub fn kernel_owns(_start: u64, _end: u64) -> bool { false }
pub fn node_line(line: u32) -> bool { (line as usize) < super::context::MSI_FIRST }
/// No console node to lend: the console is the kernel's channel.
pub fn console() -> usize { 0 }
pub fn console_line() -> Option<usize> { Some(super::serial::LINE) }
pub fn irq(_line: usize) -> bool { false }
pub unsafe fn board_from_tree(_address: u64) -> bool { true }
pub fn reserved(_each: impl FnMut(u64, u64)) {}
