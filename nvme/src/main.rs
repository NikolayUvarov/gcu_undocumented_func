#![no_std]
#![no_main]
// Ring 3 NVMe driver (issue 205): the first namespace of the first NVMe controller (PCI class 01:08:02), for the
// boards and servers whose disks are on PCIe. Registers in BAR0, an admin queue pair and one I/O queue pair in its
// own DMA region, commands polled to completion one at a time like ahci's; reads, writes (for clients with the write
// badge, mind::block_protocol) and flushes, 512-byte sectors only. Holds its BAR and DMA only.
use core::sync::atomic::{fence, Ordering};
use mind::abi::{BootInfo, BLOCK_KIND_NVME, BLOCK_MAX_SECTORS, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM};
use mind::block::{self, Driver};
use mind::dev::{cap_info, Dma, Mmio};

const CAP: usize = 0x00; const CC: usize = 0x14; const CSTS: usize = 0x1C; const AQA: usize = 0x24; const ASQ: usize = 0x28; const ACQ: usize = 0x30;
const ENTRIES: u16 = 16; // per queue: 16 commands of 64 bytes, 16 completions of 16
// DMA region layout: admin submission and completion queues, I/O ones, an identify page, the PRP list, the data.
const ADMIN_SQ: usize = 0x0000; const ADMIN_CQ: usize = 0x1000; const IO_SQ: usize = 0x2000; const IO_CQ: usize = 0x3000;
const IDENTIFY: usize = 0x4000; const PRP_LIST: usize = 0x5000; const DATA: usize = 0x10000;
const PAGE: usize = 4096;
const OP_FLUSH: u8 = 0x00; const OP_WRITE: u8 = 0x01; const OP_READ: u8 = 0x02; // I/O
const OP_CREATE_SQ: u8 = 0x01; const OP_CREATE_CQ: u8 = 0x05; const OP_IDENTIFY: u8 = 0x06; // admin

// A queue pair: its submission tail, completion head and phase, and its doorbells.
struct Queue { sq: usize, cq: usize, tail: u16, head: u16, phase: bool, sq_bell: usize, cq_bell: usize, id: u16 }

struct Nvme { regs: Mmio, dma: Dma, admin: Queue, io: Queue, sectors: u64, flush: bool }

// Waits for a condition, busy-polling first, then sleeping (QEMU completes commands asynchronously).
fn wait(mut done: impl FnMut() -> bool) -> bool {
    for attempt in 0..3_000 { if done() { return true; } if attempt > 1_000 { mind::time::sleep(1); } else { core::hint::spin_loop(); } }
    false
}

impl Nvme {
    fn probe() -> Option<Self> {
        if cap_info(SLOT_DEV0).0 != CAP_KIND_MMIO { return None; }
        let (regs, mut dma) = (Mmio::map(SLOT_DEV0).ok()?, Dma::map(SLOT_MEM).ok()?);
        if dma.len() < DATA + BLOCK_MAX_SECTORS * 512 { return None; }
        dma.zero(0, DATA);
        let cap = regs.read64(CAP);
        let stride = 4usize << (cap >> 32 & 0xF); // doorbell stride (CAP.DSTRD)
        let timeout_ms = (cap >> 24 & 0xFF) as usize * 500; // CAP.TO, in 500 ms units
        // Disable, set the admin queues, enable with 4 KiB pages and the standard entry sizes.
        regs.write32(CC, 0);
        if !wait_ms(timeout_ms, || regs.read32(CSTS) & 1 == 0) { return None; }
        regs.write32(AQA, (ENTRIES as u32 - 1) << 16 | (ENTRIES as u32 - 1));
        regs.write64(ASQ, dma.physical(ADMIN_SQ)); regs.write64(ACQ, dma.physical(ADMIN_CQ));
        regs.write32(CC, 1 | 6 << 16 | 4 << 20); // EN, IOSQES 64 bytes, IOCQES 16 bytes
        if !wait_ms(timeout_ms, || regs.read32(CSTS) & 3 == 1) { return None; } // RDY, no fatal status
        let queue = |id: u16, sq: usize, cq: usize| Queue { sq, cq, tail: 0, head: 0, phase: true, sq_bell: 0x1000 + 2 * id as usize * stride, cq_bell: 0x1000 + (2 * id as usize + 1) * stride, id };
        let mut nvme = Self { regs, dma, admin: queue(0, ADMIN_SQ, ADMIN_CQ), io: queue(1, IO_SQ, IO_CQ), sectors: 0, flush: false };
        // The controller (for the volatile write cache), then namespace 1: its size and LBA format.
        let identify = nvme.dma.physical(IDENTIFY);
        nvme.admin_command(OP_IDENTIFY, 0, identify, [1, 0, 0, 0, 0, 0])?;
        nvme.flush = nvme.dma.bytes(IDENTIFY + 525, 1)[0] & 1 != 0; // VWC
        nvme.admin_command(OP_IDENTIFY, 1, identify, [0, 0, 0, 0, 0, 0])?;
        let id = nvme.dma.bytes(IDENTIFY, 4096);
        let size = u64::from_le_bytes(id[0..8].try_into().unwrap());
        let format = (id[26] & 0xF) as usize;
        let lba_shift = id[128 + 4 * format + 2];
        if lba_shift != 9 { mind::println!("[NVME] NAMESPACE 1 HAS {}-BYTE SECTORS: ONLY 512 ARE SERVED", 1u64 << lba_shift); return None; }
        nvme.sectors = size;
        // One I/O queue pair, polled (no interrupts).
        let (cq, sq) = (nvme.dma.physical(IO_CQ), nvme.dma.physical(IO_SQ));
        nvme.admin_command(OP_CREATE_CQ, 0, cq, [(ENTRIES as u32 - 1) << 16 | 1, 1, 0, 0, 0, 0])?;
        nvme.admin_command(OP_CREATE_SQ, 0, sq, [(ENTRIES as u32 - 1) << 16 | 1, 1 << 16 | 1, 0, 0, 0, 0])?;
        (nvme.sectors != 0).then_some(nvme)
    }

    fn admin_command(&mut self, opcode: u8, nsid: u32, prp1: u64, dwords: [u32; 6]) -> Option<()> {
        let mut queue = core::mem::replace(&mut self.admin, Queue { sq: 0, cq: 0, tail: 0, head: 0, phase: true, sq_bell: 0, cq_bell: 0, id: 0 });
        let done = self.submit(&mut queue, opcode, nsid, prp1, 0, dwords);
        self.admin = queue;
        done
    }

    // Writes a command at the queue's tail, rings its doorbell and waits for its completion; Some on success.
    fn submit(&mut self, queue: &mut Queue, opcode: u8, nsid: u32, prp1: u64, prp2: u64, dwords: [u32; 6]) -> Option<()> {
        let at = queue.sq + 64 * queue.tail as usize;
        self.dma.zero(at, 64);
        let cid = queue.tail;
        self.dma.write32(at, opcode as u32 | (cid as u32) << 16);
        self.dma.write32(at + 4, nsid);
        self.dma.write64(at + 24, prp1); self.dma.write64(at + 32, prp2);
        for (i, dword) in dwords.iter().enumerate() { self.dma.write32(at + 40 + 4 * i, *dword); }
        queue.tail = (queue.tail + 1) % ENTRIES;
        fence(Ordering::SeqCst);
        self.regs.write32(queue.sq_bell, queue.tail as u32);
        let entry = queue.cq + 16 * queue.head as usize;
        let phase = queue.phase;
        let dma = &mut self.dma;
        let mut status = || { fence(Ordering::SeqCst); u32::from_le_bytes(dma.bytes(entry + 12, 4).try_into().unwrap()) };
        if !wait(|| (status() >> 16 & 1 != 0) == phase) { mind::println!("[NVME] QUEUE {} COMMAND {:#x}: NO COMPLETION", queue.id, opcode); return None; }
        let status = status();
        queue.head = (queue.head + 1) % ENTRIES;
        if queue.head == 0 { queue.phase = !queue.phase; }
        self.regs.write32(queue.cq_bell, queue.head as u32);
        (status >> 17 & 0x7FFF == 0).then_some(())
    }

    // A read or write of `count` sectors at `lba` through the data buffer: PRP1 its first page, PRP2 the second page
    // or a list of the others.
    fn transfer(&mut self, opcode: u8, lba: u64, count: usize) -> bool {
        let bytes = count * 512;
        let (first, pages) = (self.dma.physical(DATA), bytes.div_ceil(PAGE));
        let prp2 = match pages {
            0 | 1 => 0,
            2 => first + PAGE as u64,
            _ => { for page in 1..pages { self.dma.write64(PRP_LIST + 8 * (page - 1), first + (page * PAGE) as u64); } self.dma.physical(PRP_LIST) }
        };
        let mut queue = core::mem::replace(&mut self.io, Queue { sq: 0, cq: 0, tail: 0, head: 0, phase: true, sq_bell: 0, cq_bell: 0, id: 0 });
        let done = self.submit(&mut queue, opcode, 1, first, prp2, [lba as u32, (lba >> 32) as u32, count as u32 - 1, 0, 0, 0]).is_some();
        self.io = queue;
        done
    }
}

// As wait, for up to `ms` milliseconds (the controller's own timeout for enabling and disabling).
fn wait_ms(ms: usize, mut done: impl FnMut() -> bool) -> bool {
    let start = mind::time::uptime_ms();
    loop { if done() { return true; } if mind::time::uptime_ms() - start > ms.max(500) { return false; } mind::time::sleep(1); }
}

impl Driver for Nvme {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let bytes = count * 512;
        if count == 0 || !self.transfer(OP_READ, lba, count) { return false; }
        out[..bytes].copy_from_slice(self.dma.bytes(DATA, bytes));
        true
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool {
        let bytes = count * 512;
        if count == 0 { return false; }
        self.dma.bytes(DATA, bytes).copy_from_slice(&data[..bytes]);
        self.transfer(OP_WRITE, lba, count)
    }
    fn flush(&mut self) -> bool {
        if !self.flush { return true; }
        let mut queue = core::mem::replace(&mut self.io, Queue { sq: 0, cq: 0, tail: 0, head: 0, phase: true, sq_bell: 0, cq_bell: 0, id: 0 });
        let done = self.submit(&mut queue, OP_FLUSH, 1, 0, 0, [0; 6]).is_some();
        self.io = queue;
        done
    }
    fn read_only(&self) -> bool { false }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut disk = Nvme::probe();
    match &disk {
        Some(d) => mind::println!("[NVME] NAMESPACE 1: {} SECTORS{}", d.sectors, if d.flush { ", WRITE CACHE" } else { "" }),
        None => mind::println!("[NVME] NO USABLE NAMESPACE"),
    }
    block::serve(BLOCK_KIND_NVME, disk.as_mut().map(|d| d as &mut dyn Driver));
}
