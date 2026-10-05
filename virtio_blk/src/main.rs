#![no_std]
#![no_main]
// Ring 3 VirtIO block driver (issue 202): the modern interface over one BAR, one request virtqueue in its own DMA
// region, requests polled to completion one at a time like ahci's. Reads, writes (for clients with the write badge,
// mind::block_protocol) and flushes; a read-only device (VIRTIO_BLK_F_RO) refuses writes. Holds its BAR and DMA only.
use core::sync::atomic::{fence, Ordering};
use mind::abi::{BootInfo, BLOCK_KIND_VIRTIO, BLOCK_MAX_SECTORS, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM};
use mind::block::{self, Driver};
use mind::dev::{cap_info, Dma, Mmio};
use mind::virtio::{Layout, Modern, NO_VECTOR};

const F_RO: u64 = 1 << 5; const F_FLUSH: u64 = 1 << 9;
const T_IN: u32 = 0; const T_OUT: u32 = 1; const T_FLUSH: u32 = 4;
const S_OK: u8 = 0;
const DESC_NEXT: u16 = 1; const DESC_WRITE: u16 = 2;
const QUEUE: u16 = 16; // entries; one request uses three
// DMA region layout: descriptors, available ring, used ring (4 KiB aligned), request header and status, data buffer.
const AVAIL: usize = 16 * QUEUE as usize; const USED: usize = 0x1000; const HEADER: usize = 0x2000; const STATUS: usize = 0x2010;
const DATA: usize = 0x10000;

struct VirtioBlk { modern: Modern, dma: Dma, size: u16, notify: usize, last_used: u16, sectors: u64, features: u64 }

// Waits for a condition, busy-polling first, then sleeping (QEMU completes requests asynchronously).
fn wait(mut done: impl FnMut() -> bool) -> bool {
    for attempt in 0..3_000 { if done() { return true; } if attempt > 1_000 { mind::time::sleep(1); } else { core::hint::spin_loop(); } }
    false
}

impl VirtioBlk {
    fn probe() -> Option<Self> {
        if cap_info(SLOT_DEV0).0 != CAP_KIND_MMIO { return None; }
        let layout = Layout::read(SLOT_DEV0)?;
        layout.single_bar()?;
        let modern = Modern { bar: Mmio::map(SLOT_DEV0).ok()?, layout };
        let features = modern.negotiate(F_RO | F_FLUSH)?;
        let mut dma = Dma::map(SLOT_MEM).ok()?;
        if dma.len() < DATA + BLOCK_MAX_SECTORS * 512 { return None; }
        dma.zero(0, DATA);
        let (size, notify) = modern.queue(0, QUEUE, dma.physical(0), dma.physical(AVAIL), dma.physical(USED), NO_VECTOR)?;
        if size < 3 { return None; }
        let sectors = modern.bar.read64(modern.layout.device.offset as usize); // capacity, in 512-byte sectors
        modern.ready();
        Some(Self { modern, dma, size, notify, last_used: 0, sectors, features })
    }

    fn descriptor(&mut self, index: usize, address: u64, length: u32, flags: u16, next: u16) {
        let at = 16 * index;
        self.dma.write64(at, address); self.dma.write32(at + 8, length);
        self.dma.bytes(at + 12, 4).copy_from_slice(&[flags as u8, (flags >> 8) as u8, next as u8, (next >> 8) as u8]);
    }

    // One request: header, `bytes` of the data buffer (device-written for reads), status; true when it succeeded.
    fn request(&mut self, kind: u32, sector: u64, bytes: usize) -> bool {
        self.dma.write32(HEADER, kind); self.dma.write32(HEADER + 4, 0); self.dma.write64(HEADER + 8, sector);
        self.dma.bytes(STATUS, 1)[0] = 0xFF;
        let (header, data, status) = (self.dma.physical(HEADER), self.dma.physical(DATA), self.dma.physical(STATUS));
        self.descriptor(0, header, 16, DESC_NEXT, 1);
        let last = if bytes > 0 { self.descriptor(1, data, bytes as u32, DESC_NEXT | if kind == T_IN { DESC_WRITE } else { 0 }, 2); 2 } else { 1 };
        self.descriptor(last, status, 1, DESC_WRITE, 0);
        let index = u16::from_le_bytes(self.dma.bytes(AVAIL + 2, 2).try_into().unwrap());
        let slot = AVAIL + 4 + 2 * (index % self.size) as usize;
        self.dma.bytes(slot, 2).copy_from_slice(&0u16.to_le_bytes());
        fence(Ordering::SeqCst); // the chain and ring entry are visible before the index moves
        self.dma.bytes(AVAIL + 2, 2).copy_from_slice(&index.wrapping_add(1).to_le_bytes());
        fence(Ordering::SeqCst);
        self.modern.notify(self.notify, 0);
        let expected = self.last_used.wrapping_add(1);
        let dma = &mut self.dma;
        let done = wait(|| { fence(Ordering::SeqCst); u16::from_le_bytes(dma.bytes(USED + 2, 2).try_into().unwrap()) == expected });
        if !done { return false; }
        self.last_used = expected;
        fence(Ordering::SeqCst);
        self.dma.bytes(STATUS, 1)[0] == S_OK
    }
}

impl Driver for VirtioBlk {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let bytes = count * 512;
        if !self.request(T_IN, lba, bytes) { return false; }
        out[..bytes].copy_from_slice(self.dma.bytes(DATA, bytes));
        true
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool {
        let bytes = count * 512;
        self.dma.bytes(DATA, bytes).copy_from_slice(&data[..bytes]);
        self.request(T_OUT, lba, bytes)
    }
    fn flush(&mut self) -> bool { self.features & F_FLUSH == 0 || self.request(T_FLUSH, 0, 0) }
    fn read_only(&self) -> bool { self.features & F_RO != 0 }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut disk = VirtioBlk::probe();
    match &disk {
        Some(d) => mind::println!("[VIRTIO_BLK] {} SECTORS, QUEUE {}{}", d.sectors, d.size, if d.read_only() { ", READ ONLY" } else { "" }),
        None => mind::println!("[VIRTIO_BLK] NO DEVICE"),
    }
    block::serve(BLOCK_KIND_VIRTIO, disk.as_mut().map(|d| d as &mut dyn Driver));
}
