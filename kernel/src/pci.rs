// PCI devices for ring 3 drivers: enumeration, BARs, decoding, MSI-X. Configuration access, the legacy interrupt
// line and the MSI address are the architecture's (arch/*/pcicfg.rs, issue 202).
use crate::pcicfg::{read, write};

#[derive(Clone, Copy, Default)]
pub struct Bar { pub base: u64, pub size: u64, pub io: bool }

#[derive(Clone, Copy)]
pub struct Device { pub class: u32, pub id: u32, pub bars: [Bar; 6], pub irq: u8, bus: u8, device: u8, function: u8, pub granted: bool, pub windows: [(u64, u64); 2] }

impl Device {
    /// PCI location as bus << 8 | device << 3 | function (observation only: configuration space stays the kernel's).
    pub fn location(&self) -> u32 { (self.bus as u32) << 8 | (self.device as u32) << 3 | self.function as u32 }
}

// BAR size is determined by writing all ones with decoding disabled; the command register is restored afterwards.
// A bridge (header type 1) has two BARs: what follows them is its bus numbers and windows.
unsafe fn bars(bus: u8, device: u8, function: u8) -> [Bar; 6] {
    let mut result = [Bar::default(); 6];
    let count = match (read(bus, device, function, 0x0C) >> 16) & 0x7F { 0 => 6, 1 => 2, _ => 0 };
    let command = read(bus, device, function, 0x04);
    write(bus, device, function, 0x04, command & !0x3);
    let mut index = 0;
    while index < count {
        let offset = 0x10 + index as u8 * 4;
        let original = read(bus, device, function, offset);
        write(bus, device, function, offset, 0xFFFF_FFFF);
        let mask = read(bus, device, function, offset);
        write(bus, device, function, offset, original);
        if original & 1 != 0 {
            let size = (!(mask & 0xFFFF_FFFC) as u64 + 1) & 0xFFFF;
            result[index] = Bar { base: (original & 0xFFFF_FFFC) as u64, size, io: true };
        } else if mask != 0 {
            let wide = (original >> 1) & 3 == 2;
            let (mut base, mut size_mask) = ((original & 0xFFFF_FFF0) as u64, (mask & 0xFFFF_FFF0) as u64 | 0xFFFF_FFFF_0000_0000);
            if wide && index + 1 < count {
                let high = read(bus, device, function, offset + 4);
                write(bus, device, function, offset + 4, 0xFFFF_FFFF);
                let high_mask = read(bus, device, function, offset + 4);
                write(bus, device, function, offset + 4, high);
                base |= (high as u64) << 32; size_mask = (size_mask & 0xFFFF_FFFF) | (high_mask as u64) << 32;
            }
            result[index] = Bar { base, size: !size_mask + 1, io: false };
            if wide { index += 1; }
        }
        index += 1;
    }
    write(bus, device, function, 0x04, command);
    result
}

// A bridge's memory windows (base, end): non-prefetchable and prefetchable; (0, 0) for one it does not forward.
unsafe fn windows(bus: u8, device: u8, function: u8) -> [(u64, u64); 2] {
    if (read(bus, device, function, 0x0C) >> 16) & 0x7F != 1 { return [(0, 0); 2]; }
    let memory = read(bus, device, function, 0x20);
    let (base, limit) = (((memory & 0xFFF0) as u64) << 16, ((memory >> 16 & 0xFFF0) as u64) << 16 | 0xF_FFFF);
    let prefetch = read(bus, device, function, 0x24);
    let wide = prefetch & 0xF == 1;
    let high = |offset| if wide { (read(bus, device, function, offset) as u64) << 32 } else { 0 };
    let (pbase, plimit) = (((prefetch & 0xFFF0) as u64) << 16 | high(0x28), ((prefetch >> 16 & 0xFFF0) as u64) << 16 | 0xF_FFFF | high(0x2C));
    let window = |b: u64, l: u64| if l > b { (b, l + 1) } else { (0, 0) };
    [window(base, limit), window(pbase, plimit)]
}

// Intel 7, 8 and 9 series chipsets (from 2012, Intel Macs among them) give their USB ports to the EHCI controllers
// until the system hands them to the xHCI one (issue 164): Panther Point, Lynx Point (and LP), Wildcat Point (and LP).
const INTEL_SWITCHABLE_XHCI: [u32; 5] = [0x1E31_8086, 0x8C31_8086, 0x9C31_8086, 0x8CB1_8086, 0x9CB1_8086];

// USB 3 SuperSpeed on every port that has it (USB3_PSSEN from USB3PRM), USB 2 to xHCI (XUSB2PR from XUSB2PRM).
unsafe fn route_to_xhci(bus: u8, device: u8, function: u8) {
    write(bus, device, function, 0xD8, read(bus, device, function, 0xDC));
    write(bus, device, function, 0xD0, read(bus, device, function, 0xD4));
    // The ports the firmware lets move (its masks) and those that moved: a port left on EHCI is not seen (211-PRT-0004).
    let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!(
        "MIND CORE KERNEL: PCI: INTEL XHCI: USB PORTS ROUTED FROM EHCI TO XHCI (USB 2 {:X} OF MASK {:X}, USB 3 {:X} OF MASK {:X})\n",
        read(bus, device, function, 0xD0), read(bus, device, function, 0xD4), read(bus, device, function, 0xD8), read(bus, device, function, 0xDC)));
}

// EHCI (211-DRV-0004): firmware with legacy USB support owns the controller through the USBLEGSUP semaphore, in
// configuration space at HCCPARAMS.EECP; the system takes it before a driver runs, as drivers cannot write there.
unsafe fn ehci_handoff(bus: u8, device: u8, function: u8, bar: Bar) {
    if bar.io || bar.base == 0 || bar.base >= 1 << 32 { return; }
    let command = read(bus, device, function, 0x04);
    write(bus, device, function, 0x04, command | 0x2); // memory decoding, for HCCPARAMS
    let hcc = core::ptr::read_volatile((bar.base + 8) as *const u32);
    write(bus, device, function, 0x04, command);
    let eecp = (hcc >> 8) as u8 & 0xFC;
    if eecp < 0x40 || read(bus, device, function, eecp) & 0xFF != 1 { return; }
    write(bus, device, function, eecp, read(bus, device, function, eecp) | 1 << 24); // OS owned
    let released = (0..50_000_000).any(|_| { core::hint::spin_loop(); read(bus, device, function, eecp) & 1 << 16 == 0 });
    write(bus, device, function, eecp + 4, 0); // no SMIs from the controller
    let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: PCI: EHCI {:02X}:{:02X}.{}: {}\n", bus, device, function,
        if released { "TAKEN FROM THE FIRMWARE" } else { "THE FIRMWARE DID NOT LET GO" }));
}

// All PCI functions with their class code, BARs and legacy IRQ line; decoding is not enabled here.
pub unsafe fn enumerate() -> alloc::vec::Vec<Device> {
    let mut devices = alloc::vec::Vec::new();
    let Some(last) = crate::pcicfg::last_bus() else { return devices };
    for bus in 0..=last {
        for device in 0..32u8 {
            if read(bus, device, 0, 0) & 0xFFFF == 0xFFFF { continue; }
            let functions = if read(bus, device, 0, 0x0C) & 0x0080_0000 != 0 { 8 } else { 1 };
            for function in 0..functions {
                if read(bus, device, function, 0) & 0xFFFF == 0xFFFF { continue; }
                let class = read(bus, device, function, 0x08) >> 8;
                let id = read(bus, device, function, 0); // vendor | device << 16
                let irq = crate::pcicfg::line(bus, device, function);
                if class == 0x0C_03_30 && INTEL_SWITCHABLE_XHCI.contains(&id) { route_to_xhci(bus, device, function); }
                let bars = bars(bus, device, function);
                if class == 0x0C_03_20 { ehci_handoff(bus, device, function, bars[0]); }
                devices.push(Device { class, id, bars, irq, bus, device, function, granted: false, windows: windows(bus, device, function) });
            }
        }
    }
    devices
}

// Moves memory BAR `index` to `base` (below 4 GiB) with decoding off; false if the device did not take it.
pub unsafe fn move_bar(device: &mut Device, index: usize, base: u64) -> bool {
    let (bus, dev, function, offset) = (device.bus, device.device, device.function, 0x10 + index as u8 * 4);
    let command = read(bus, dev, function, 0x04);
    write(bus, dev, function, 0x04, command & !0x2);
    let original = read(bus, dev, function, offset);
    write(bus, dev, function, offset, (base as u32 & 0xFFFF_FFF0) | (original & 0xF));
    if (original >> 1) & 3 == 2 && index < 5 { write(bus, dev, function, offset + 4, (base >> 32) as u32); }
    let taken = read(bus, dev, function, offset) & 0xFFFF_FFF0 == base as u32 & 0xFFFF_FFF0;
    if !taken { write(bus, dev, function, offset, original); }
    write(bus, dev, function, 0x04, command);
    if taken { device.bars[index].base = base; }
    taken
}

// Stops a device before its driver is restarted: no decoding, no bus mastering (no DMA).
pub unsafe fn quiesce(device: &Device) {
    let command = read(device.bus, device.device, device.function, 0x04);
    write(device.bus, device.device, device.function, 0x04, command & !0x7);
}

// Enables I/O, memory decoding and bus mastering once a resource of the device is handed to a driver.
pub unsafe fn enable(device: &Device) {
    let command = read(device.bus, device.device, device.function, 0x04);
    write(device.bus, device.device, device.function, 0x04, command | 0x7);
}

/// A dword of the device's configuration space (offset < 256, aligned down to 4); reading has no side effects.
pub unsafe fn config(device: &Device, offset: u8) -> u32 { read(device.bus, device.device, device.function, offset) }

// Offset of the device's capability `id` in configuration space.
unsafe fn capability(device: &Device, id: u8) -> Option<u8> {
    if config(device, 0x04) >> 16 & 0x10 == 0 { return None; }
    let mut at = config(device, 0x34) as u8 & 0xFC;
    for _ in 0..48 {
        if at < 0x40 { return None; }
        let header = config(device, at);
        if header as u8 == id { return Some(at); }
        at = (header >> 8) as u8 & 0xFC;
    }
    None
}

/// Address of MSI-X table entry `entry` of the device (in a memory BAR below 4 GiB), or None.
pub unsafe fn msix_entry(device: &Device, entry: u16) -> Option<u64> {
    let cap = capability(device, 0x11)?;
    let control = config(device, cap) >> 16;
    if entry as u32 > control & 0x7FF { return None; }
    let table = config(device, cap + 4);
    let bar = *device.bars.get((table & 7) as usize)?;
    let at = bar.base.checked_add((table & !7) as u64 + 16 * entry as u64)?;
    (!bar.io && bar.size != 0 && at + 16 <= crate::mmu::IDENTITY_END && at + 16 <= bar.base + bar.size).then_some(at)
}

/// Points MSI-X table entry `entry` (mapped uncached by the caller) at MSI line `index` (the arch's message), unmasks
/// it and enables MSI-X, which turns the legacy line off. None where the platform has no MSI target.
pub unsafe fn msix(device: &Device, entry: u16, index: usize) -> Option<()> {
    let (target, data) = crate::pcicfg::msi_message(device.location(), index)?;
    let cap = capability(device, 0x11)?;
    let header = config(device, cap);
    let control = header >> 16;
    if entry as u32 > control & 0x7FF { return None; }
    let at = msix_entry(device, entry)?;
    enable(device);
    let entry = at as *mut u32;
    entry.write_volatile(target as u32); entry.add(1).write_volatile((target >> 32) as u32); entry.add(2).write_volatile(data); entry.add(3).write_volatile(0);
    write(device.bus, device.device, device.function, cap, (header & 0xFFFF) | ((control | 0x8000) & !0x4000) << 16);
    Some(())
}
