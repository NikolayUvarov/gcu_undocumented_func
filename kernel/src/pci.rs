use crate::{inl, outl};

// Minimal PCI configuration access (mechanism #1) for handing devices to ring 3 drivers.
unsafe fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    outl(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset as u32 & 0xFC));
    inl(0xCFC)
}
unsafe fn write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    outl(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset as u32 & 0xFC));
    outl(0xCFC, value);
}

#[derive(Clone, Copy, Default)]
pub struct Bar { pub base: u64, pub size: u64, pub io: bool }

#[derive(Clone, Copy)]
pub struct Device { pub bars: [Bar; 6], pub irq: u8 }

// BAR size is determined by writing all ones with decoding disabled, then the value is restored.
unsafe fn bars(bus: u8, device: u8, function: u8) -> [Bar; 6] {
    let mut result = [Bar::default(); 6];
    let command = read(bus, device, function, 0x04);
    write(bus, device, function, 0x04, command & !0x3);
    let mut index = 0;
    while index < 6 {
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
            if wide && index < 5 {
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
    write(bus, device, function, 0x04, command | 0x7); // I/O, memory, bus mastering
    result
}

// First device whose class code (class<<16 | subclass<<8 | interface) matches under the mask.
pub unsafe fn find(class: u32, mask: u32) -> Option<Device> {
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            if read(bus, device, 0, 0) & 0xFFFF == 0xFFFF { continue; }
            let functions = if read(bus, device, 0, 0x0C) & 0x0080_0000 != 0 { 8 } else { 1 };
            for function in 0..functions {
                if read(bus, device, function, 0) & 0xFFFF == 0xFFFF { continue; }
                let code = read(bus, device, function, 0x08) >> 8;
                if code & mask != class { continue; }
                let irq = read(bus, device, function, 0x3C) as u8;
                return Some(Device { bars: bars(bus, device, function), irq: if irq < 16 { irq } else { 0 } });
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
pub struct Ac97 { pub mixer: u16, pub bus_master: u16, pub irq: u8 }

// AC97: class 04:01, two I/O BARs and an IRQ line.
pub unsafe fn find_ac97() -> Option<Ac97> {
    find(0x04_01_00, 0xFF_FF_00)
        .filter(|d| d.bars[0].io && d.bars[1].io && d.irq != 0)
        .map(|d| Ac97 { mixer: d.bars[0].base as u16, bus_master: d.bars[1].base as u16, irq: d.irq })
}
