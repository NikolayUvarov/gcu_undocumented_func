//! Stage-1 translation of user addresses through the kernel's own page tables (215-PRT-0016): the aarch64 format with
//! a 4 KiB granule, 48-bit addresses and four levels, the user's permissions checked on every access, and a small TLB
//! that the kernel flushes when it changes a table or switches TTBR0. A physical address outside RAM is a device's.
use crate::{Access, Bus, Fault, FaultKind};

/// What the page offers besides RAM: device registers, entropy and the clock.
pub trait Devices {
    fn read(&mut self, _pa: u64, _size: u32) -> Option<u64> { None }
    fn write(&mut self, _pa: u64, _size: u32, _value: u64) -> bool { false }
    fn random(&mut self) -> Option<u64> { None }
    fn counter(&mut self) -> u64 { 0 }
    fn frequency(&mut self) -> u64 { 0 }
}

const ENTRIES: usize = 64;
const ADDRESS: u64 = 0x0000_FFFF_FFFF_F000;

#[derive(Clone, Copy)]
struct Entry { page: u64, frame: u64, level: u8, read: bool, write: bool, exec: bool }

const EMPTY: Entry = Entry { page: u64::MAX, frame: 0, level: 0, read: false, write: false, exec: false };

/// Translations kept between runs; one per CPU.
pub struct Tlb { entries: [Entry; ENTRIES] }

impl Tlb {
    pub const fn new() -> Self { Self { entries: [EMPTY; ENTRIES] } }
    pub fn flush(&mut self) { self.entries = [EMPTY; ENTRIES]; }
}

impl Default for Tlb { fn default() -> Self { Self::new() } }

/// A task's view of memory: RAM from `ram_base`, the task's level-0 table at `ttbr0`.
pub struct Mmu<'a, D: Devices> {
    pub ram: &'a mut [u8],
    pub ram_base: u64,
    pub ttbr0: u64,
    pub tlb: &'a mut Tlb,
    pub devices: &'a mut D,
}

impl<'a, D: Devices> Mmu<'a, D> {
    fn ram_range(&self, pa: u64, size: u32) -> Option<usize> {
        let offset = pa.checked_sub(self.ram_base)?;
        let end = offset.checked_add(size as u64)?;
        (end <= self.ram.len() as u64).then_some(offset as usize)
    }

    fn physical_read(&mut self, pa: u64, size: u32) -> Option<u64> {
        match self.ram_range(pa, size) {
            Some(at) => {
                let mut bytes = [0u8; 8];
                bytes[..size as usize].copy_from_slice(&self.ram[at..at + size as usize]);
                Some(u64::from_le_bytes(bytes))
            }
            None => self.devices.read(pa, size),
        }
    }

    fn physical_write(&mut self, pa: u64, size: u32, value: u64) -> bool {
        match self.ram_range(pa, size) {
            Some(at) => { self.ram[at..at + size as usize].copy_from_slice(&value.to_le_bytes()[..size as usize]); true }
            None => self.devices.write(pa, size, value),
        }
    }

    /// Walks the tables for `va`: the page's frame and what EL0 may do there, or why there is no translation.
    fn walk(&mut self, va: u64) -> Result<Entry, FaultKind> {
        if va >> 48 != 0 { return Err(FaultKind::Translation(0)); }
        let (mut table, mut el0, mut writable, mut executable) = (self.ttbr0 & ADDRESS, true, true, true);
        for level in 0u8..4 {
            let shift = 39 - 9 * level as u32;
            let index = (va >> shift) & 0x1FF;
            // Tables are read from RAM only, never from a device.
            let at = self.ram_range(table + index * 8, 8).ok_or(FaultKind::Translation(level))?;
            let descriptor = u64::from_le_bytes(self.ram[at..at + 8].try_into().unwrap());
            if descriptor & 1 == 0 { return Err(FaultKind::Translation(level)); }
            if level < 3 && descriptor & 2 != 0 {
                // APTable bit 61: no EL0 access below; bit 62: no writes below. UXNTable, bit 60.
                if descriptor >> 61 & 1 == 1 { el0 = false; }
                if descriptor >> 62 & 1 == 1 { writable = false; }
                if descriptor >> 60 & 1 == 1 { executable = false; }
                table = descriptor & ADDRESS;
                continue;
            }
            if level == 0 || (level == 3 && descriptor & 2 == 0) { return Err(FaultKind::Translation(level)); }
            if descriptor & (1 << 10) == 0 { return Err(FaultKind::AccessFlag(level)); }
            let ap = (descriptor >> 6) & 3;
            let el0 = el0 && ap & 1 == 1;
            let span = (1u64 << shift) - 1;
            let frame = (descriptor & ADDRESS & !span) | (va & span & !0xFFF);
            return Ok(Entry {
                page: va >> 12,
                frame,
                level,
                read: el0,
                write: el0 && writable && ap & 2 == 0,
                exec: el0 && executable && descriptor >> 54 & 1 == 0,
            });
        }
        Err(FaultKind::Translation(3))
    }

    /// The physical address of `va` for `access`, or the fault.
    pub fn translate(&mut self, va: u64, access: Access) -> Result<u64, Fault> {
        let page = va >> 12;
        let slot = page as usize % ENTRIES;
        let mut entry = self.tlb.entries[slot];
        if entry.page != page {
            entry = self.walk(va).map_err(|kind| Fault { address: va, access, kind })?;
            self.tlb.entries[slot] = entry;
        }
        let allowed = match access { Access::Read => entry.read, Access::Write => entry.write, Access::Fetch => entry.exec };
        if !allowed { return Err(Fault { address: va, access, kind: FaultKind::Permission(entry.level) }); }
        Ok(entry.frame | (va & 0xFFF))
    }
}

impl<'a, D: Devices> Bus for Mmu<'a, D> {
    fn read(&mut self, va: u64, size: u32, access: Access) -> Result<u64, Fault> {
        if (va & 0xFFF) + size as u64 > 0x1000 {
            let mut v = 0;
            for i in 0..size as u64 { v |= self.read(va.wrapping_add(i), 1, access)? << (8 * i); }
            return Ok(v);
        }
        let pa = self.translate(va, access)?;
        self.physical_read(pa, size).ok_or(Fault { address: va, access, kind: FaultKind::Device })
    }

    fn write(&mut self, va: u64, size: u32, value: u64) -> Result<(), Fault> {
        if (va & 0xFFF) + size as u64 > 0x1000 {
            // Both pages translated before either is written, so a fault changes nothing.
            self.translate(va, Access::Write)?;
            self.translate((va | 0xFFF).wrapping_add(1), Access::Write)?; // the second page's first byte, as a fault names it
            for i in 0..size as u64 { self.write(va.wrapping_add(i), 1, value >> (8 * i))?; }
            return Ok(());
        }
        let pa = self.translate(va, Access::Write)?;
        if self.physical_write(pa, size, value) { Ok(()) } else { Err(Fault { address: va, access: Access::Write, kind: FaultKind::Device }) }
    }

    fn random(&mut self) -> Option<u64> { self.devices.random() }
    fn counter(&mut self) -> u64 { self.devices.counter() }
    fn frequency(&mut self) -> u64 { self.devices.frequency() }
}
