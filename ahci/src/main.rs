#![no_std]
#![no_main]
// Ring 3 AHCI (SATA) driver: HBA registers via an MMIO capability, commands and data in its own DMA region. READ DMA
// EXT, WRITE DMA EXT and FLUSH CACHE EXT (writes only for clients with the write badge, see mind::block_protocol).
use mind::abi::{BootInfo, BLOCK_KIND_AHCI, SLOT_DEV0, SLOT_MEM};
use mind::block::{self, Driver};
use mind::dev::{Dma, Mmio};

const CAP2: usize = 0x24; const BOHC: usize = 0x28; const GHC: usize = 0x04; const PI: usize = 0x0C;
const CLB: usize = 0x00; const FB: usize = 0x08; const IS: usize = 0x10; const CMD: usize = 0x18; const TFD: usize = 0x20;
const SIG: usize = 0x24; const SSTS: usize = 0x28; const SERR: usize = 0x30; const CI: usize = 0x38;
const CMD_ST: u32 = 1; const CMD_SUD: u32 = 2; const CMD_POD: u32 = 4; const CMD_FRE: u32 = 1 << 4; const CMD_FR: u32 = 1 << 14; const CMD_CR: u32 = 1 << 15;
const IS_TFES: u32 = 1 << 30;
// DMA region layout: command list, received FIS, command table, data buffer (64 KiB aligned).
const LIST: usize = 0; const FIS: usize = 0x400; const TABLE: usize = 0x1000; const DATA: usize = 0x10000;

struct Ahci { hba: Mmio, dma: Dma, port: usize, sectors: u64 }

// Waits for a condition, busy-polling first, then sleeping (QEMU completes DMA asynchronously).
fn wait(mut done: impl FnMut() -> bool) -> bool {
    for attempt in 0..2_000 { if done() { return true; } if attempt > 1_000 { mind::time::sleep(10); } else { core::hint::spin_loop(); } }
    false
}

impl Ahci {
    fn reg(&self, offset: usize) -> u32 { self.hba.read32(0x100 + self.port * 0x80 + offset) }
    fn set(&self, offset: usize, value: u32) { self.hba.write32(0x100 + self.port * 0x80 + offset, value) }

    fn probe() -> Option<Self> {
        let (hba, dma) = (Mmio::map(SLOT_DEV0).ok()?, Dma::map(SLOT_MEM).ok()?);
        hba.write32(GHC, hba.read32(GHC) | 1 << 31); // AHCI mode
        if hba.read32(CAP2) & 1 != 0 { hba.write32(BOHC, hba.read32(BOHC) | 2); wait(|| hba.read32(BOHC) & 1 == 0); } // take the controller from the BIOS
        let implemented = hba.read32(PI);
        let port = (0..32).find(|&p| implemented & 1 << p != 0 && hba.read32(0x100 + p * 0x80 + SSTS) & 0xF == 3 && hba.read32(0x100 + p * 0x80 + SIG) == 0x0000_0101)?;
        let mut device = Self { hba, dma, port, sectors: 0 };
        device.start()?;
        let mut identify = [0u8; 512];
        device.command(0xEC, 0, 0, &mut identify)?;
        let word = |i: usize| u16::from_le_bytes([identify[i * 2], identify[i * 2 + 1]]) as u64;
        let lba48 = word(100) | word(101) << 16 | word(102) << 32 | word(103) << 48;
        device.sectors = if lba48 != 0 { lba48 } else { word(60) | word(61) << 16 };
        (device.sectors != 0).then_some(device)
    }

    fn start(&mut self) -> Option<()> {
        self.set(CMD, self.reg(CMD) & !CMD_ST);
        if !wait(|| self.reg(CMD) & CMD_CR == 0) { return None; }
        self.set(CMD, self.reg(CMD) & !CMD_FRE);
        if !wait(|| self.reg(CMD) & CMD_FR == 0) { return None; }
        self.dma.zero(0, DATA);
        self.set(CLB, self.dma.physical(LIST) as u32); self.set(CLB + 4, (self.dma.physical(LIST) >> 32) as u32);
        self.set(FB, self.dma.physical(FIS) as u32); self.set(FB + 4, (self.dma.physical(FIS) >> 32) as u32);
        self.set(SERR, u32::MAX); self.set(IS, u32::MAX);
        self.set(CMD, self.reg(CMD) | CMD_FRE | CMD_SUD | CMD_POD);
        self.set(CMD, self.reg(CMD) | CMD_ST);
        wait(|| self.reg(TFD) & 0x88 == 0).then_some(())
    }

    // One command in slot 0: H2D FIS and, with data, one PRDT entry for the data buffer; `write` sends the buffer.
    fn issue(&mut self, command: u8, lba: u64, count: usize, bytes: usize, write: bool) -> Option<()> {
        let table = self.dma.physical(TABLE); let data = self.dma.physical(DATA);
        self.dma.zero(TABLE, 0x100);
        let fis = self.dma.bytes(TABLE, 20);
        fis[0] = 0x27; fis[1] = 0x80; fis[2] = command; fis[7] = 0x40;
        for i in 0..3 { fis[4 + i] = (lba >> (8 * i)) as u8; fis[8 + i] = (lba >> (24 + 8 * i)) as u8; }
        fis[12] = count as u8; fis[13] = (count >> 8) as u8;
        let entries = if bytes > 0 { self.dma.write64(TABLE + 0x80, data); self.dma.write32(TABLE + 0x8C, bytes as u32 - 1); 1 } else { 0 };
        // Header: FIS length 5 dwords, W (bit 6) for host-to-device data, PRDT entries.
        self.dma.write32(LIST, 5 | if write { 1 << 6 } else { 0 } | entries << 16); self.dma.write32(LIST + 4, 0); self.dma.write64(LIST + 8, table);
        self.set(IS, u32::MAX);
        self.set(CI, 1);
        let finished = wait(|| self.reg(CI) & 1 == 0 || self.reg(IS) & IS_TFES != 0);
        (finished && self.reg(IS) & IS_TFES == 0 && self.reg(TFD) & 1 == 0).then_some(())
    }

    // A command that reads into `out`.
    fn command(&mut self, command: u8, lba: u64, count: usize, out: &mut [u8]) -> Option<()> {
        let bytes = out.len();
        self.issue(command, lba, count, bytes, false)?;
        out.copy_from_slice(self.dma.bytes(DATA, bytes));
        Some(())
    }
}

impl Driver for Ahci {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool { self.command(0x25, lba, count, &mut out[..count * 512]).is_some() } // READ DMA EXT
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool { // WRITE DMA EXT
        let bytes = count * 512;
        self.dma.bytes(DATA, bytes).copy_from_slice(&data[..bytes]);
        self.issue(0x35, lba, count, bytes, true).is_some()
    }
    fn flush(&mut self) -> bool { self.issue(0xEA, 0, 0, 0, false).is_some() } // FLUSH CACHE EXT
    fn read_only(&self) -> bool { false }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut disk = Ahci::probe();
    match &disk {
        Some(d) => mind::println!("[AHCI] PORT {}: {} SECTORS", d.port, d.sectors),
        None => mind::println!("[AHCI] NO SATA DISK"),
    }
    block::serve(BLOCK_KIND_AHCI, disk.as_mut().map(|d| d as &mut dyn Driver));
}
