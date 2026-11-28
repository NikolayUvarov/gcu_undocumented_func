// No PCI in a page yet (215-PRT-0017): the devices come later, as virtio from the page.
pub fn ecam() -> Option<(u64, u64)> { None }
pub fn last_bus() -> Option<u8> { None }
pub unsafe fn line(_bus: u8, _device: u8, _function: u8) -> u8 { 0 }
pub unsafe fn msi_message(_location: u32, _index: usize) -> Option<(u64, u32)> { None }
/// Every function reads as absent.
pub unsafe fn read(_bus: u8, _device: u8, _function: u8, _offset: u8) -> u32 { u32::MAX }
pub unsafe fn write(_bus: u8, _device: u8, _function: u8, _offset: u8, _value: u32) {}
