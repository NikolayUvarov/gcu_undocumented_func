#[path = "../rtw89_wifi/src/io.rs"]
#[allow(dead_code)] mod io;
#[path = "../rtw89_wifi/src/fwcmd.rs"]
#[allow(dead_code)] mod fwcmd;

use fwcmd::{Bus, Error, Queue, State};
use io::{Fault, FaultKind, RegisterIo, Result};
use std::alloc::{alloc_zeroed, dealloc, Layout};

// Raw memory observations below simulate the coherent DMA device, not a concurrent CPU alias.
struct Region { ptr: *mut u8, layout: Layout }
impl Region {
    fn new() -> Self {
        let layout = Layout::from_size_align(fwcmd::DMA_BYTES, 4096).unwrap();
        let ptr = unsafe { alloc_zeroed(layout) }; assert!(!ptr.is_null()); Self { ptr, layout }
    }
    fn word(&self, at: usize) -> u32 { unsafe { u32::from_le(std::ptr::read_volatile(self.ptr.add(at).cast::<u32>())) } }
    fn bytes(&self, at: usize, len: usize) -> Vec<u8> { (at..at + len).map(|i| unsafe { std::ptr::read_volatile(self.ptr.add(i)) }).collect() }
}
impl Drop for Region { fn drop(&mut self) { unsafe { dealloc(self.ptr, self.layout); } } }

struct Device {
    registers: Vec<u8>, command: u16, now: u64, frozen: bool, writes: Vec<(usize, usize, u32)>,
    bad_readback: bool, fail_doorbell: bool, memory: *mut u8, observed: Vec<Vec<u8>>,
}
impl Device {
    fn new() -> Self {
        let mut d = Self { registers: vec![0; 0x2000], command: 2, now: 0, frozen: false, writes: Vec::new(),
            bad_readback: false, fail_doorbell: false, memory: std::ptr::null_mut(), observed: Vec::new() };
        d.put(fwcmd::DMA_STOP, 4, 1 << 18); d
    }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.registers[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }
    fn get(&self, at: usize, width: usize) -> u32 {
        let mut value = [0; 4]; value[..width].copy_from_slice(&self.registers[at..at + width]); u32::from_le_bytes(value)
    }
    fn running(&mut self) { self.command = 7; self.put(fwcmd::INIT_CONFIG, 4, 1 << 11); self.put(fwcmd::DMA_STOP, 4, 0); }
    fn complete(&mut self, position: usize) {
        let value = self.get(fwcmd::INDEX, 4) & 0xffff;
        self.put(fwcmd::INDEX, 4, value | (position as u32) << 16);
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> Result<()> {
        io::check_offset(at, width, self.registers.len())?;
        if at == fwcmd::INDEX && width == 2 && !self.memory.is_null() {
            let descriptor = ((value as usize + 31) % 32) * 8;
            let length = unsafe { std::ptr::read_volatile(self.memory.add(descriptor).cast::<u16>()) } as usize;
            let address = unsafe { std::ptr::read_volatile(self.memory.add(descriptor + 4).cast::<u32>()) };
            let offset = address as usize - 0x200000;
            assert!(offset + length <= fwcmd::DMA_BYTES);
            self.observed.push((offset..offset + length).map(|i| unsafe { std::ptr::read_volatile(self.memory.add(i)) }).collect());
        }
        self.put(at, width, value); self.writes.push((at, width, value));
        if at == fwcmd::CLEAR_INDEX { self.put(fwcmd::INDEX, 4, 0); }
        if self.fail_doorbell && at == fwcmd::INDEX { return Err(Fault::new(FaultKind::Access, at, value)); }
        Ok(())
    }
}
impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> Result<u8> { Ok(self.get(at, 1) as u8) }
    fn read16(&mut self, at: usize) -> Result<u16> { Ok(self.get(at, 2) as u16) }
    fn read32(&mut self, at: usize) -> Result<u32> {
        if self.bad_readback && at == fwcmd::BASE_LOW { return Ok(0); }
        Ok(self.get(at, 4))
    }
    fn write8(&mut self, at: usize, value: u8) -> Result<()> { self.write(at, 1, value as u32) }
    fn write16(&mut self, at: usize, value: u16) -> Result<()> { self.write(at, 2, value as u32) }
    fn write32(&mut self, at: usize, value: u32) -> Result<()> { self.write(at, 4, value) }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, value: u32) -> Result<()> { if !self.frozen { self.now += value as u64; } Ok(()) }
}
impl Bus for Device { fn pci_command(&mut self) -> Result<u16> { Ok(self.command) } }

fn queue<'a>(region: &'a Region, bus: &mut Device) -> Queue<'a> {
    bus.memory = region.ptr;
    unsafe { Queue::new(bus, region.ptr, fwcmd::DMA_BYTES, 0x200000) }.unwrap()
}
fn running<'a>(region: &'a Region, bus: &mut Device) -> Queue<'a> {
    let mut q = queue(region, bus); q.install(bus).unwrap(); bus.running(); unsafe { q.start(bus) }.unwrap(); q
}

#[test]
fn rejects_bad_dma_bounds_alignment_or_active_master_before_memory_writes() {
    let region = Region::new(); let mut bus = Device::new();
    unsafe { std::ptr::write_volatile(region.ptr, 0x55); }
    for (ptr, len, physical) in [
        (std::ptr::null_mut(), fwcmd::DMA_BYTES, 0x200000),
        (unsafe { region.ptr.add(1) }, fwcmd::DMA_BYTES - 1, 0x200000),
        (region.ptr, fwcmd::DMA_BYTES - 1, 0x200000),
        (region.ptr, fwcmd::DMA_BYTES, 0),
        (region.ptr, fwcmd::DMA_BYTES, 0x200001),
        (region.ptr, fwcmd::DMA_BYTES, 0xfffff000),
        (region.ptr, fwcmd::DMA_BYTES, u64::MAX),
    ] { assert!(matches!(unsafe { Queue::new(&mut bus, ptr, len, physical) }, Err(Error::Region))); }
    bus.command = 6;
    assert!(matches!(unsafe { Queue::new(&mut bus, region.ptr, fwcmd::DMA_BYTES, 0x200000) }, Err(Error::State)));
    assert_eq!(region.bytes(0, 1), [0x55]); assert!(bus.writes.is_empty());
}

#[test]
fn installation_never_enables_dma_and_checks_queue_registers() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = queue(&region, &mut bus);
    q.install(&mut bus).unwrap();
    assert_eq!(q.state(), State::Installed); assert_eq!(q.physical(), 0x200000);
    assert_eq!(bus.command, 2);
    assert_eq!(bus.get(fwcmd::NUMBER, 2), 32);
    assert_eq!(bus.get(fwcmd::BDRAM, 4), 0x1041c);
    assert_eq!(bus.get(fwcmd::BASE_LOW, 4), 0x200000);
    assert_eq!(bus.get(fwcmd::BASE_HIGH, 4), 0);
    assert!(bus.writes.iter().all(|(at, _, _)| ![fwcmd::INIT_CONFIG, fwcmd::DMA_STOP].contains(at)));
    assert_eq!(q.submit(&mut bus, b"x", false), Err(Error::State));
}

#[test]
fn refuses_install_when_master_or_hci_or_channel_is_active() {
    for case in 0..4 {
        let region = Region::new(); let mut bus = Device::new(); let mut q = queue(&region, &mut bus);
        match case { 0 => bus.command = 7, 1 => bus.put(fwcmd::INIT_CONFIG, 4, 1 << 11),
            2 => bus.put(fwcmd::INIT_CONFIG, 4, 1 << 13), _ => bus.put(fwcmd::DMA_STOP, 4, 0) }
        assert_eq!(q.install(&mut bus), Err(Error::State)); assert!(bus.writes.is_empty());
        assert_eq!(q.state(), State::Faulted);
    }
}

#[test]
fn busy_device_or_failed_readback_poison_installation() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = queue(&region, &mut bus);
    bus.put(fwcmd::DMA_BUSY, 4, 1 << 18);
    assert!(matches!(q.install(&mut bus), Err(Error::Io(Fault { kind: FaultKind::Timeout, .. }))));
    assert!(bus.now <= 100); assert!(bus.writes.is_empty());
    let region = Region::new(); let mut bus = Device::new(); let mut q = queue(&region, &mut bus);
    bus.bad_readback = true;
    assert_eq!(q.install(&mut bus), Err(Error::State)); assert_eq!(q.state(), State::Faulted);
    bus.running(); assert_eq!(unsafe { q.start(&mut bus) }, Err(Error::State));
}

#[test]
fn start_requires_installed_indices_address_and_external_engine_configuration() {
    for case in 0..7 {
        let region = Region::new(); let mut bus = Device::new(); let mut q = queue(&region, &mut bus);
        if case != 0 { q.install(&mut bus).unwrap(); bus.running(); }
        match case {
            1 => bus.command = 2, 2 => bus.put(fwcmd::INIT_CONFIG, 4, 0),
            3 => bus.put(fwcmd::DMA_STOP, 4, 1 << 18), 4 => bus.put(fwcmd::BASE_LOW, 4, 0x300000),
            5 => bus.put(fwcmd::INDEX, 4, 1 << 16), 6 => bus.command = u16::MAX, _ => (),
        }
        let writes = bus.writes.len(); assert_eq!(unsafe { q.start(&mut bus) }, Err(Error::State));
        assert_eq!(bus.writes.len(), writes);
    }
}

#[test]
fn command_and_download_descriptors_are_complete_before_doorbell() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    q.submit(&mut bus, &[0xab; 2020], true).unwrap();
    assert_eq!(region.word(0), 0x400007fc); assert_eq!(region.word(4), 0x201000);
    assert_eq!(region.word(4096), 0x001c0000); assert_eq!(region.word(4104), 2020);
    assert_eq!(&bus.observed[0][24..], &[0xab; 2020]);
    assert!(bus.observed[0][4..8].iter().chain(bus.observed[0][12..24].iter()).all(|b| *b == 0));
    q.submit(&mut bus, b"H2C test", false).unwrap();
    assert_eq!(region.word(4096 + 2048), 0x000c0000);
    assert_eq!(region.word(8), 0x40000020); assert_eq!(region.word(12), 0x201800);
    assert_eq!(bus.writes.last(), Some(&(fwcmd::INDEX, 2, 2)));
}

#[test]
fn reserves_one_descriptor_and_does_not_overwrite_a_full_queue() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    for n in 0..31 { q.submit(&mut bus, &[n], false).unwrap(); }
    let before = region.bytes(0, fwcmd::DMA_BYTES); let writes = bus.writes.len();
    assert_eq!(q.submit(&mut bus, b"full", true), Err(Error::Full));
    assert_eq!(q.pending(), 31); assert_eq!(q.state(), State::Running);
    assert_eq!(region.bytes(0, fwcmd::DMA_BYTES), before); assert_eq!(bus.writes.len(), writes);
}

#[test]
fn eight_completed_payloads_survive_descriptor_wrap_and_reuse() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    for n in 0..31 { q.submit(&mut bus, &[n; 16], true).unwrap(); }
    bus.complete(31); assert_eq!(q.reclaim(&mut bus).unwrap(), 31);
    assert_eq!(q.retained(), 8);
    let retained: Vec<_> = (23..31).map(|n| region.bytes(4096 + n * 2048, 40)).collect();
    for n in 31..62 { q.submit(&mut bus, &[n; 16], true).unwrap(); }
    for (n, old) in (23..31).zip(retained) { assert_eq!(region.bytes(4096 + n * 2048, 40), old); }
    assert_eq!(q.pending(), 31); assert_eq!(bus.get(fwcmd::INDEX, 2), 30);
}

#[test]
fn repeated_wrap_keeps_all_last_eight_completed_payloads_immutable() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    for n in 0..300usize {
        q.submit(&mut bus, &(n as u32).to_le_bytes(), true).unwrap();
        for old in n.saturating_sub(8)..n {
            assert_eq!(region.word(4096 + (old % 40) * 2048 + 24), old as u32);
        }
        bus.complete((n + 1) % 32); assert_eq!(q.reclaim(&mut bus).unwrap(), 1);
        assert_eq!(q.pending(), 0);
    }
}

#[test]
fn invalid_hardware_or_host_index_poison_without_reusing_payloads() {
    for raw in [32 << 16 | 1, 2 << 16 | 1, 31 << 16 | 1, 1 << 16 | 2, u32::MAX] {
        let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
        q.submit(&mut bus, b"held", true).unwrap(); let before = region.bytes(4096, 32);
        bus.put(fwcmd::INDEX, 4, raw);
        assert!(q.reclaim(&mut bus).is_err()); assert_eq!(q.state(), State::Faulted);
        assert_eq!(q.submit(&mut bus, b"bad", true), Err(Error::State));
        assert_eq!(region.bytes(4096, 32), before);
    }
}

#[test]
fn a_doorbell_error_retains_a_possibly_published_command() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    bus.fail_doorbell = true;
    assert!(matches!(q.submit(&mut bus, b"held", true), Err(Error::Io(_))));
    assert_eq!(q.state(), State::Faulted); assert_eq!(q.pending(), 1);
    assert_eq!(region.bytes(4096 + 24, 4), b"held"); assert_eq!(bus.get(fwcmd::INDEX, 2), 1);
}

#[test]
fn drain_timeout_and_frozen_clock_keep_dma_and_disable_further_submission() {
    for frozen in [false, true] {
        let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
        q.submit(&mut bus, b"held", true).unwrap(); bus.frozen = frozen;
        assert!(matches!(q.drain(&mut bus, 1000), Err(Error::Io(Fault { kind: FaultKind::Timeout, .. }))));
        assert_eq!(q.pending(), 1); assert_eq!(q.state(), State::Faulted); assert!(bus.now <= 1000);
        assert_eq!(region.bytes(4096 + 24, 4), b"held"); assert_eq!(bus.command, 7);
    }
}

#[test]
fn drain_does_not_release_the_final_prefetch_margin() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    for n in 0..12 { q.submit(&mut bus, &[n], false).unwrap(); }
    bus.complete(12); q.drain(&mut bus, 1000).unwrap();
    assert_eq!(q.pending(), 0); assert_eq!(q.retained(), 8); assert_eq!(q.state(), State::Running);
    for n in 4..12 { assert_eq!(region.bytes(4096 + n * 2048 + 24, 1), [n as u8]); }
    assert_eq!(bus.command, 7);
}

#[test]
fn invalid_packet_lengths_do_not_mutate_queue_or_device() {
    let region = Region::new(); let mut bus = Device::new(); let mut q = running(&region, &mut bus);
    let writes = bus.writes.len();
    assert_eq!(q.submit(&mut bus, &[], false), Err(Error::Length));
    assert_eq!(q.submit(&mut bus, &[0; 2021], true), Err(Error::Length));
    assert_eq!(q.pending(), 0); assert_eq!(q.state(), State::Running); assert_eq!(bus.writes.len(), writes);
}
