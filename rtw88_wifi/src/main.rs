#![no_std]
#![no_main]

//! rtw88_wifi (550-DRV-0042): the RTL8821CE's Wi-Fi. This first stage checks what init granted, reads the chip's PCI
//! identity and the firmware file, and maps BAR2; the chip's bring-up follows in later stages.

use mind::abi::{BootInfo, CAP_KIND_DMA, CAP_KIND_ENDPOINT, CAP_KIND_MMIO, SLOT_DEV0, SLOT_MEM, SLOT_VFS};
use mind::dev::{cap_info, device_config, Mmio};
use mind::mem::Pages;

/// The firmware, as scripts/firmware/rtl8821ce.py fetches it and scripts/proprietary.sh copies it.
const FIRMWARE_PATH: &str = "data/firmware/rtw88/rtw8821c_fw.bin";
const FIRMWARE_BYTES: usize = 139_472;
const FIRMWARE_SHA256: [u8; 32] = [
    0x2e, 0xf4, 0x09, 0xbc, 0x41, 0x85, 0x49, 0xfc, 0xf2, 0x94, 0x06, 0x1d, 0xd0, 0xca, 0xe1, 0xfc,
    0x22, 0xfd, 0x9d, 0xa7, 0x9b, 0x60, 0x52, 0x49, 0x50, 0xb2, 0x5d, 0xe1, 0x87, 0x32, 0xf3, 0xf0,
];
/// The RTL8821CE (rtw8821ce.c): Realtek, device 0xc821.
const PCI_ID: u32 = 0xc821_10ec;

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

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (kind, _, len) = cap_info(SLOT_DEV0);
    if kind != CAP_KIND_MMIO { mind::println!("[RTW88] NO MMIO BAR2 GRANT"); idle(); }
    let (Ok(id), Ok(command), Ok(revision)) = (device_config(SLOT_DEV0, 0), device_config(SLOT_DEV0, 4), device_config(SLOT_DEV0, 8)) else {
        mind::println!("[RTW88] PCI CONFIG UNAVAILABLE");
        idle();
    };
    mind::println!("[RTW88] PCI {:04X}:{:04X} REVISION {:02X} COMMAND {:04X}; BAR2 {} BYTES", id & 0xffff, id >> 16, revision & 0xff, command & 0xffff, len);
    if id != PCI_ID { mind::println!("[RTW88] NOT AN RTL8821CE: NO DEVICE START"); idle(); }
    if cap_info(SLOT_MEM).0 != CAP_KIND_DMA { mind::println!("[RTW88] NO DMA REGION GRANT"); idle(); }
    let file = match firmware() {
        Ok(file) => file,
        Err(error) => { mind::println!("[RTW88] FIRMWARE {} UNAVAILABLE OR REJECTED: {:?}; NO DEVICE START", FIRMWARE_PATH, error); idle(); }
    };
    let Ok(bar) = Mmio::map(SLOT_DEV0) else { mind::println!("[RTW88] BAR2 MAP FAILED"); idle(); };
    let header = file.as_slice();
    mind::println!("[RTW88] FIRMWARE {} BYTES, SIGNATURE {:02X}{:02X}, VERSION {}.{}; SYSTEM ON {:08X}",
        header.len(), header[1], header[0], u16::from_le_bytes([header[4], header[5]]), header[6], bar.read32(0));
    mind::println!("[RTW88] THE CHIP'S BRING-UP IS NOT WRITTEN YET (550-DRV-0042)");
    idle()
}

fn idle() -> ! { loop { mind::time::sleep(60_000); } }
