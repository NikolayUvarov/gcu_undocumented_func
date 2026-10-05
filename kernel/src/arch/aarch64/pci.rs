// PCI on aarch64 comes with ECAM (issue 202); until then no devices are enumerated.
#[derive(Clone, Copy, Debug, Default)]
pub struct Bar { pub base: u64, pub size: u64, pub io: bool }
#[derive(Clone, Copy, Debug, Default)]
pub struct Device { pub class: u32, pub id: u32, pub bars: [Bar; 6], pub irq: u8 }
impl Device {
    pub fn location(&self) -> u32 { 0 }
}
pub unsafe fn enumerate() -> alloc::vec::Vec<Device> { alloc::vec::Vec::new() }
pub unsafe fn quiesce(_device: &Device) {}
pub unsafe fn enable(_device: &Device) {}
pub unsafe fn config(_device: &Device, _offset: u8) -> u32 { u32::MAX }
pub unsafe fn msix_entry(_device: &Device, _entry: u16) -> Option<u64> { None }
pub unsafe fn msix(_device: &Device, _entry: u16, _cpu: u32, _vector: u8) -> Option<()> { None }
