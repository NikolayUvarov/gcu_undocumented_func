use crate::{inl, outl};

// Минимальный доступ к конфигурации PCI (механизм #1) для раздачи устройств драйверам ring 3.
unsafe fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    outl(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset as u32 & 0xFC));
    inl(0xCFC)
}
unsafe fn write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    outl(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset as u32 & 0xFC));
    outl(0xCFC, value);
}

#[derive(Clone, Copy)]
pub struct Ac97 { pub mixer: u16, pub bus_master: u16, pub irq: u8 }

// Ищет AC97 (класс 04:01 с двумя I/O BAR), включает I/O и bus mastering.
pub unsafe fn find_ac97() -> Option<Ac97> {
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            if read(bus, device, 0, 0) & 0xFFFF == 0xFFFF { continue; }
            let functions = if read(bus, device, 0, 0x0C) & 0x0080_0000 != 0 { 8 } else { 1 };
            for function in 0..functions {
                let id = read(bus, device, function, 0);
                if id & 0xFFFF == 0xFFFF { continue; }
                let class = read(bus, device, function, 0x08) >> 16;
                let (bar0, bar1) = (read(bus, device, function, 0x10), read(bus, device, function, 0x14));
                if class != 0x0401 || bar0 & 1 == 0 || bar1 & 1 == 0 { continue; }
                let command = read(bus, device, function, 0x04);
                write(bus, device, function, 0x04, command | 0x5);
                let irq = read(bus, device, function, 0x3C) as u8;
                if irq == 0 || irq >= 16 { return None; }
                return Some(Ac97 { mixer: (bar0 & 0xFFFC) as u16, bus_master: (bar1 & 0xFFFC) as u16, irq });
            }
        }
    }
    None
}
