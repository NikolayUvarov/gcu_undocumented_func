// PCI under a page (215-PRT-0017): bus 0 alone, with the page's two virtio-input devices (`wasm_machine`).
pub fn ecam() -> Option<(u64, u64)> { None }
pub fn last_bus() -> Option<u8> { Some(0) }
/// The device's line: the input devices' own, else none.
pub unsafe fn line(_bus: u8, device: u8, _function: u8) -> u8 { if (1..=2).contains(&device) { wasm_machine::LINES[device as usize - 1] } else { 0 } }
pub unsafe fn msi_message(_location: u32, _index: usize) -> Option<(u64, u32)> { None }
pub unsafe fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 { super::run::machine().config_read(bus, device, function, offset) }
pub unsafe fn write(bus: u8, device: u8, function: u8, offset: u8, value: u32) { super::run::machine().config_write(bus, device, function, offset, value) }
