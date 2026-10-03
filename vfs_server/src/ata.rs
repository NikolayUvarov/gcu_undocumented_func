// Первичный канал ATA в режиме PIO LBA28 без прерываний (nIEN), с небольшим кэшем секторов.
use mind::abi::{SLOT_DEV0, SLOT_DEV1};
use mind::dev::Ports;

const DATA: u16 = 0x1F0; const COUNT: u16 = 0x1F2; const LBA0: u16 = 0x1F3; const LBA1: u16 = 0x1F4; const LBA2: u16 = 0x1F5;
const DRIVE: u16 = 0x1F6; const COMMAND: u16 = 0x1F7; const CONTROL: u16 = 0x3F6;
const BSY: u8 = 0x80; const DRQ: u8 = 0x08; const ERR: u8 = 0x01;
const CACHE: usize = 16;

pub struct Disk { io: Ports, control: Ports, cache: [(u32, [u8; 512]); CACHE], next: usize }

impl Disk {
    pub fn probe() -> Option<Self> {
        let (io, control) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1));
        control.out8(CONTROL, 0x02);
        io.out8(DRIVE, 0xA0);
        if io.in8(COMMAND) == 0xFF { return None; } // плавающая шина: контроллера нет
        for port in [COUNT, LBA0, LBA1, LBA2] { io.out8(port, 0); }
        io.out8(COMMAND, 0xEC);
        if io.in8(COMMAND) == 0 { return None; }
        let mut disk = Self { io, control, cache: [(u32::MAX, [0; 512]); CACHE], next: 0 };
        let mut spins = 0; while io.in8(COMMAND) & BSY != 0 { spins += 1; if spins > 1_000_000 { return None; } }
        if io.in8(LBA1) != 0 || io.in8(LBA2) != 0 { return None; } // ATAPI/SATA: не наш случай
        disk.wait_data()?;
        let mut identify = [0u16; 256];
        io.read_words(DATA, &mut identify).ok()?;
        Some(disk)
    }

    fn wait_data(&mut self) -> Option<()> {
        for _ in 0..4 { self.control.in8(CONTROL); } // задержка 400 нс по альтернативному статусу
        for _ in 0..1_000_000 {
            let status = self.io.in8(COMMAND);
            if status & BSY != 0 { continue; }
            if status & ERR != 0 { return None; }
            if status & DRQ != 0 { return Some(()); }
        }
        None
    }

    pub fn read(&mut self, lba: u32) -> Option<&[u8; 512]> {
        if let Some(index) = self.cache.iter().position(|(cached, _)| *cached == lba) { return Some(&self.cache[index].1); }
        while self.io.in8(COMMAND) & BSY != 0 {}
        self.io.out8(DRIVE, 0xE0 | ((lba >> 24) & 0x0F) as u8);
        self.io.out8(COUNT, 1);
        self.io.out8(LBA0, lba as u8); self.io.out8(LBA1, (lba >> 8) as u8); self.io.out8(LBA2, (lba >> 16) as u8);
        self.io.out8(COMMAND, 0x20);
        self.wait_data()?;
        let mut words = [0u16; 256];
        self.io.read_words(DATA, &mut words).ok()?;
        let index = self.next; self.next = (self.next + 1) % CACHE;
        self.cache[index].0 = lba;
        for (i, word) in words.iter().enumerate() { self.cache[index].1[i * 2..i * 2 + 2].copy_from_slice(&word.to_le_bytes()); }
        Some(&self.cache[index].1)
    }
}
