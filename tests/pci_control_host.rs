#[path = "../kernel/src/pci_control.rs"]
mod control;
use control::{Access, Mode, transition};

struct Pci { value: u32, written: Vec<u16>, refuse: bool }
impl Access for Pci {
    fn read(&mut self) -> u32 { self.value }
    fn write_command(&mut self, value: u16) {
        self.written.push(value);
        if !self.refuse { self.value = self.value & 0xffff_0000 | u32::from(value); }
    }
}

#[test]
fn registers_only_stop_and_dma_preserve_status_and_other_command_bits() {
    let mut pci = Pci { value: 0xf900_0547, written: vec![], refuse: false };
    assert!(transition(&mut pci, Mode::Registers));
    assert_eq!(pci.value, 0xf900_0542);
    assert!(transition(&mut pci, Mode::Dma { decode: 3 }));
    assert_eq!(pci.value, 0xf900_0547);
    assert!(transition(&mut pci, Mode::Stopped));
    assert_eq!(pci.value, 0xf900_0540);
    assert_eq!(pci.written, [0x0542, 0x0547, 0x0540]);
}

#[test]
fn a_controller_that_keeps_mastering_is_not_reported_stopped() {
    let mut pci = Pci { value: 7, written: vec![], refuse: true };
    assert!(!transition(&mut pci, Mode::Stopped));
    assert!(!transition(&mut pci, Mode::Registers));
}

#[test]
fn a_controller_that_refuses_mastering_is_not_reported_started() {
    let mut pci = Pci { value: 2, written: vec![], refuse: true };
    assert!(!transition(&mut pci, Mode::Dma { decode: 3 }));
}

#[test]
fn inaccessible_configuration_is_never_written() {
    let mut pci = Pci { value: u32::MAX, written: vec![], refuse: false };
    assert!(!transition(&mut pci, Mode::Registers));
    assert!(pci.written.is_empty());
}

extern crate alloc;
#[allow(dead_code)]
#[path = "../kernel/src/pci.rs"]
mod pci;

struct PanicSerial;
impl core::fmt::Write for PanicSerial { fn write_str(&mut self, _: &str) -> core::fmt::Result { Ok(()) } }
mod mmu { pub const IDENTITY_END: u64 = u64::MAX; }
mod clock { pub fn spin_us(_: u64) {} }

mod pcicfg {
    use std::cell::RefCell;
    pub struct Config { pub words: [u32; 64], pub writes: Vec<(u8, u32)>, pub command_mask: u16 }
    thread_local! { pub static CONFIG: RefCell<Config> = const { RefCell::new(Config { words: [0; 64], writes: Vec::new(), command_mask: u16::MAX }) }; }
    pub fn last_bus() -> Option<u8> { Some(0) }
    pub fn line(_: u8, _: u8, _: u8) -> u8 { 255 }
    pub fn msi_message(_: u32, index: usize) -> Option<(u64, u32)> { (index < 16).then_some((0xfee0_0000, 0x50 + index as u32)) }
    pub unsafe fn read(bus: u8, device: u8, function: u8, at: u8) -> u32 {
        if (bus, device, function) != (0, 0, 0) { return u32::MAX; }
        CONFIG.with(|config| {
            let config = config.borrow(); let value = config.words[at as usize / 4];
            if (0x10..0x28).contains(&at) && value == u32::MAX { if at == 0x10 { 0xffff_f000 } else { 0 } } else { value }
        })
    }
    pub unsafe fn write(_: u8, _: u8, _: u8, at: u8, value: u32) {
        CONFIG.with(|config| {
            let mut config = config.borrow_mut(); config.writes.push((at, value));
            if at == 4 {
                let status = config.words[1] & 0xffff_0000 & !(value & 0xf900_0000);
                config.words[1] = status | u32::from(value as u16 & config.command_mask);
            } else { config.words[at as usize / 4] = value; }
        });
    }
}

fn device_fixture(command: u16, command_mask: u16) -> pci::Device {
    pcicfg::CONFIG.with(|config| {
        let mut config = config.borrow_mut(); config.words = [0; 64]; config.writes.clear();
        config.command_mask = u16::MAX;
        config.words[0] = 0x8168_10ec; config.words[1] = 0x0010_0000;
        config.words[2] = 0x0200_000c; config.words[4] = 0x1000;
        config.words[0x34 / 4] = 0x40;
    });
    let mut devices = unsafe { pci::enumerate() };
    assert_eq!(devices.len(), 1);
    pcicfg::CONFIG.with(|config| {
        let mut config = config.borrow_mut(); config.words[1] = 0xf910_0000 | u32::from(command);
        config.command_mask = command_mask; config.writes.clear();
    });
    devices.remove(0)
}

#[test]
fn actual_enable_derives_decode_spaces_and_accepts_hardwired_unused_bits() {
    for (io, wanted, mask) in [(false, 6, !1u16), (true, 5, !2u16)] {
        let mut device = device_fixture(0x0400, mask);
        device.bars[0].io = io;
        assert!(unsafe { pci::enable(&device) });
        pcicfg::CONFIG.with(|config| {
            let config = config.borrow();
            assert_eq!(config.words[1], 0xf910_0400 | wanted);
            assert_eq!(config.writes, [(4, 0x0400 | wanted)]);
        });
        assert!(unsafe { pci::quiesce(&device) });
        pcicfg::CONFIG.with(|config| assert_eq!(config.borrow().words[1], 0xf910_0400));
    }
}

#[test]
fn actual_msi_configures_address_and_vector_without_any_command_write() {
    for command in [0u16, 2, 6, 7] {
        let device = device_fixture(command, u16::MAX);
        pcicfg::CONFIG.with(|config| config.borrow_mut().words[0x40 / 4] = 0x0080_0005);
        assert_eq!(unsafe { pci::msi(&device, 3) }, Some(()));
        pcicfg::CONFIG.with(|config| {
            let config = config.borrow();
            assert_eq!(config.words[1], 0xf910_0000 | u32::from(command));
            assert!(!config.writes.iter().any(|&(at, _)| at == 4));
            assert_eq!(config.words[0x44 / 4], 0xfee0_0000);
            assert_eq!(config.words[0x48 / 4], 0);
            assert_eq!(config.words[0x4c / 4], 0x53);
            assert_eq!(config.words[0x40 / 4] >> 16 & 0x71, 1);
        });
    }
}

#[test]
fn actual_msix_requires_memory_decode_and_keeps_mastering_unchanged() {
    for command in [0u16, 1, 4, 5, 2, 6] {
        let mut device = device_fixture(command, u16::MAX);
        let mut table = [0xa5a5_a5a5u32; 4];
        device.bars[0].base = table.as_mut_ptr() as u64; device.bars[0].size = 16;
        pcicfg::CONFIG.with(|config| {
            let mut config = config.borrow_mut(); config.words[0x40 / 4] = 0x11; config.words[0x44 / 4] = 0;
        });
        let configured = unsafe { pci::msix(&device, 0, 1) };
        if command & 2 == 0 {
            assert_eq!(configured, None); assert_eq!(table, [0xa5a5_a5a5; 4]);
            pcicfg::CONFIG.with(|config| assert!(config.borrow().writes.is_empty()));
        } else {
            assert_eq!(configured, Some(())); assert_eq!(table, [0xfee0_0000, 0, 0x51, 0]);
            pcicfg::CONFIG.with(|config| assert_eq!(config.borrow().words[0x40 / 4] >> 16 & 0xc000, 0x8000));
        }
        pcicfg::CONFIG.with(|config| {
            let config = config.borrow();
            assert_eq!(config.words[1], 0xf910_0000 | u32::from(command));
            assert!(!config.writes.iter().any(|&(at, _)| at == 4));
        });
    }
}

#[test]
fn actual_interrupt_setup_rejects_inaccessible_command_without_writes() {
    let device = device_fixture(2, u16::MAX);
    pcicfg::CONFIG.with(|config| config.borrow_mut().words[1] = u32::MAX);
    assert_eq!(unsafe { pci::msi(&device, 0) }, None);
    assert_eq!(unsafe { pci::msix(&device, 0, 0) }, None);
    pcicfg::CONFIG.with(|config| assert!(config.borrow().writes.is_empty()));
}
