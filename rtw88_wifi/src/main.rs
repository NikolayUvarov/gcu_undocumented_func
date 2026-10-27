#![no_std]
#![no_main]

//! rtw88_wifi (550-DRV-0042): the RTL8821CE's Wi-Fi as an idl/wlanhw.wit radio. It checks what init granted, reads the
//! firmware, brings the chip up in rtw88's order with the rings polled, and serves the station.

mod serve;

use core::mem::ManuallyDrop;
use mind::abi::{BootInfo, CAP_KIND_DMA, CAP_KIND_ENDPOINT, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM, SLOT_VFS};
use mind::dev::{cap_info, device_config, Dma, Mmio};
use mind::mem::Pages;
use rtw88_wifi::dma::{self, Region};
use rtw88_wifi::io::Regs;
use rtw88_wifi::radio::{self, QUEUE_SLOT};

/// The firmware, as scripts/firmware/rtl8821ce.py fetches it and scripts/proprietary.sh copies it.
const FIRMWARE_PATH: &str = "data/firmware/rtw88/rtw8821c_fw.bin";
const FIRMWARE_BYTES: usize = 139_472;
const FIRMWARE_SHA256: [u8; 32] = [
    0x2e, 0xf4, 0x09, 0xbc, 0x41, 0x85, 0x49, 0xfc, 0xf2, 0x94, 0x06, 0x1d, 0xd0, 0xca, 0xe1, 0xfc,
    0x22, 0xfd, 0x9d, 0xa7, 0x9b, 0x60, 0x52, 0x49, 0x50, 0xb2, 0x5d, 0xe1, 0x87, 0x32, 0xf3, 0xf0,
];
/// The RTL8821CE (rtw8821ce.c): Realtek, device 0xc821.
const PCI_ID: u32 = 0xc821_10ec;
/// Received frames the station has not taken yet.
const QUEUE: usize = 32;

// The fields are read through Debug, in the log.
#[allow(dead_code)]
#[derive(Debug)]
enum FileError { Grant, Open(mind::fs::Error), Size(usize), Memory, Read(mind::sys::Error), Short, Hash }

/// The firmware file read whole within 5 s and checked by its size and SHA-256.
fn firmware() -> Result<Pages, FileError> {
    if cap_info(SLOT_VFS).0 != CAP_KIND_ENDPOINT { return Err(FileError::Grant); }
    mind::idl::wire::with_timeout(2_000, || {
        let mut file = mind::fs::File::open(FIRMWARE_PATH).map_err(FileError::Open)?;
        if file.size() != FIRMWARE_BYTES { return Err(FileError::Size(file.size())); }
        let mut bytes = Pages::new(FIRMWARE_BYTES).ok_or(FileError::Memory)?;
        let deadline = mind::deadline::Deadline::after(mind::time::monotonic_ns() / 1_000_000, 5_000);
        for chunk in bytes.as_mut_slice().chunks_mut(mind::fs::CHUNK) {
            let count = deadline.call(|| file.read(chunk).map_err(Into::into)).map_err(FileError::Read)?;
            if count != chunk.len() { return Err(FileError::Short); }
        }
        if mind::sha256::digest(bytes.as_slice()) != FIRMWARE_SHA256 { return Err(FileError::Hash); }
        Ok(bytes)
    })
}

/// BAR2's registers; short waits spun on the monotonic clock, long ones slept.
struct Native { bar: Mmio }

impl Regs for Native {
    fn r8(&mut self, at: u32) -> u8 { self.bar.read8(at as usize) }
    fn r16(&mut self, at: u32) -> u16 { self.bar.read16(at as usize) }
    fn r32(&mut self, at: u32) -> u32 { self.bar.read32(at as usize) }
    fn w8(&mut self, at: u32, value: u8) { self.bar.write8(at as usize, value) }
    fn w16(&mut self, at: u32, value: u16) { self.bar.write16(at as usize, value) }
    fn w32(&mut self, at: u32, value: u32) { self.bar.write32(at as usize, value) }
    fn udelay(&mut self, us: u32) {
        let start = mind::time::monotonic_ns();
        while mind::time::monotonic_ns().saturating_sub(start) < us as u64 * 1000 { core::hint::spin_loop(); }
    }
    // The scheduler sleeps in 10 ms ticks: shorter waits are spun.
    fn msleep(&mut self, ms: u32) { if ms < 10 { self.udelay(ms * 1000) } else { let _ = mind::time::sleep(ms as usize); } }
}

fn say(args: core::fmt::Arguments) { mind::println!("[RTW88] {}", args); }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (kind, _, len) = cap_info(SLOT_DEV0);
    if kind != CAP_KIND_MMIO { say(format_args!("NO MMIO BAR2 GRANT")); serve::absent(); }
    let (Ok(id), Ok(command), Ok(revision)) = (device_config(SLOT_DEV0, 0), device_config(SLOT_DEV0, 4), device_config(SLOT_DEV0, 8)) else {
        say(format_args!("PCI CONFIG UNAVAILABLE"));
        serve::absent();
    };
    say(format_args!("PCI {:04X}:{:04X} REVISION {:02X} COMMAND {:04X}; BAR2 {} BYTES", id & 0xffff, id >> 16, revision & 0xff, command & 0xffff, len));
    if id != PCI_ID { say(format_args!("NOT AN RTL8821CE: NO DEVICE START")); serve::absent(); }
    let Ok(index) = mind::process::args_str().trim().parse::<usize>() else { say(format_args!("NO PCI DEVICE INDEX")); serve::absent(); };
    if cap_info(SLOT_MEM).0 != CAP_KIND_DMA { say(format_args!("NO DMA REGION GRANT")); serve::absent(); }
    let file = match firmware() {
        Ok(file) => file,
        Err(error) => { say(format_args!("FIRMWARE {} UNAVAILABLE OR REJECTED: {:?}; NO DEVICE START", FIRMWARE_PATH, error)); serve::absent(); }
    };
    let Ok(bar) = Mmio::map(SLOT_DEV0) else { say(format_args!("BAR2 MAP FAILED")); serve::absent(); };
    // The region stays mapped for the process's life: the chip may still use it after any failure.
    let mut dma = match Dma::map(SLOT_MEM) { Ok(d) => ManuallyDrop::new(d), Err(e) => { say(format_args!("DMA MAP FAILED: {:?}", e)); serve::absent(); } };
    let (dma_len, phys) = (dma.len(), dma.physical(0));
    let base = dma.bytes(0, dma_len).as_mut_ptr();
    let region = match unsafe { Region::new(base, phys, dma_len) } {
        Ok(region) => region,
        Err(e) => { say(format_args!("DMA REGION {:X}, {} BYTES, REFUSED: {:?}", phys, dma_len, e)); serve::absent(); }
    };
    say(format_args!("DMA REGION {:X}, {} BYTES ({} USED)", phys, dma_len, dma::END));
    let Some(mut queue) = Pages::new(QUEUE * QUEUE_SLOT) else { say(format_args!("NO MEMORY FOR THE RECEIVE QUEUE")); serve::absent(); };
    let master = || mind::platform::resume(index).is_ok() && device_config(SLOT_DEV0, 4).is_ok_and(|c| c & 4 != 0);
    let mut log = |args: core::fmt::Arguments| say(args);
    match radio::up(Native { bar }, region, file.as_slice(), queue.as_mut_slice(), master, &mut log) {
        Ok(mut radio) => serve::run(&mut radio),
        Err((stage, error)) => {
            say(format_args!("BRING-UP STOPPED AT {:?}: {:?}", stage, error));
            serve::absent();
        }
    }
}
