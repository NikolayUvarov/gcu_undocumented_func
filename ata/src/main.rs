#![no_std]
#![no_main]
// Ring 3 primary-channel ATA driver: PIO LBA28 without interrupts (nIEN), block protocol for vfs_server.
use mind::abi::{BootInfo, BLOCK_KIND_ATA, SLOT_DEV0, SLOT_DEV1};
use mind::block::{self, Driver};
use mind::dev::Ports;

const DATA: u16 = 0x1F0; const COUNT: u16 = 0x1F2; const LBA0: u16 = 0x1F3; const LBA1: u16 = 0x1F4; const LBA2: u16 = 0x1F5;
const DRIVE: u16 = 0x1F6; const COMMAND: u16 = 0x1F7; const CONTROL: u16 = 0x3F6;
const BSY: u8 = 0x80; const DRQ: u8 = 0x08; const ERR: u8 = 0x01;

struct Ata { io: Ports, control: Ports, sectors: u64 }

impl Ata {
    fn probe() -> Option<Self> {
        let (io, control) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1));
        control.out8(CONTROL, 0x02);
        io.out8(DRIVE, 0xA0);
        if matches!(io.in8(COMMAND), 0xFF | 0x00) { return None; } // floating bus or no device
        for port in [COUNT, LBA0, LBA1, LBA2] { io.out8(port, 0); }
        io.out8(COMMAND, 0xEC);
        if io.in8(COMMAND) == 0 { return None; }
        let mut disk = Self { io, control, sectors: 0 };
        let mut spins = 0; while io.in8(COMMAND) & BSY != 0 { spins += 1; if spins > 1_000_000 { return None; } }
        if io.in8(LBA1) != 0 || io.in8(LBA2) != 0 { return None; } // ATAPI/SATA signature: not our case
        disk.wait_data()?;
        let mut identify = [0u16; 256];
        io.read_words(DATA, &mut identify).ok()?;
        disk.sectors = (identify[60] as u64 | (identify[61] as u64) << 16).min(1 << 28);
        (disk.sectors != 0).then_some(disk)
    }

    fn wait_data(&self) -> Option<()> {
        for _ in 0..4 { self.control.in8(CONTROL); } // 400 ns delay via the alternate status register
        for _ in 0..1_000_000 {
            let status = self.io.in8(COMMAND);
            if status & BSY != 0 { continue; }
            if status & ERR != 0 { return None; }
            if status & DRQ != 0 { return Some(()); }
        }
        None
    }
}

impl Driver for Ata {
    fn sectors(&self) -> u64 { self.sectors }
    // Batches of up to 256 sectors per READ SECTORS command; each sector is a separate DRQ.
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        while self.io.in8(COMMAND) & BSY != 0 {}
        self.io.out8(DRIVE, 0xE0 | ((lba >> 24) & 0x0F) as u8);
        self.io.out8(COUNT, count as u8);
        self.io.out8(LBA0, lba as u8); self.io.out8(LBA1, (lba >> 8) as u8); self.io.out8(LBA2, (lba >> 16) as u8);
        self.io.out8(COMMAND, 0x20);
        let mut words = [0u16; 256];
        for sector in out.chunks_exact_mut(512).take(count) {
            if self.wait_data().is_none() || self.io.read_words(DATA, &mut words).is_err() { return false; }
            for (i, word) in words.iter().enumerate() { sector[i * 2..i * 2 + 2].copy_from_slice(&word.to_le_bytes()); }
        }
        true
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut disk = Ata::probe();
    match &disk {
        Some(d) => mind::println!("[ATA] PRIMARY MASTER: {} SECTORS", d.sectors),
        None => mind::println!("[ATA] NO DISK ON PRIMARY CHANNEL"),
    }
    block::serve(BLOCK_KIND_ATA, disk.as_mut().map(|d| d as &mut dyn Driver));
}
