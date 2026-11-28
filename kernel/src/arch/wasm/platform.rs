// The page's platform (215-PRT-0017): no I/O ports, no nodes, no lines by number; the console's channel on line 1.
pub fn ports(_start: usize, _end: usize) -> bool { false }
pub fn nodes() { unsafe { crate::platform_nodes::build(|_table| {}) } }
pub fn kernel_owns(_start: u64, _end: u64) -> bool { false }
pub fn node_line(line: u32) -> bool { (line as usize) < super::context::MSI_FIRST }
/// No console node to lend: the console is the kernel's channel.
pub fn console() -> usize { 0 }
pub fn console_line() -> Option<usize> { Some(super::serial::LINE) }
pub fn irq(_line: usize) -> bool { false }
pub unsafe fn board_from_tree(_address: u64) -> bool { true }
pub fn reserved(_each: impl FnMut(u64, u64)) {}
