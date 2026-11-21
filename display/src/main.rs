#![no_std]
#![no_main]
// display (213-DRV-0086, DRV's): the Ryzen's Radeon display engine. This stub, KRN's (213-KRN-0270, 0271), shows the grant
// init gives: the windows of BAR 5 and BAR 0, the VBIOS image, the device's PCI identity and command register. It reads
// no register and writes nothing: only the VBIOS copy, which is RAM, is read.
use mind::abi::{BootInfo, CAP_KIND_MEMORY, CAP_KIND_MMIO, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, device_config};

// init's slots for display (213-KRN-0271): the engine's registers, the SMU mailbox's page, the second screen's range of
// BAR 0, and the VBIOS image (the ACPI VFCT table, read-only) where an interrupt would be: the display has none of its own.
const ENGINE: usize = SLOT_DEV0;
const MAILBOX: usize = SLOT_DEV1;
const SECOND_SCREEN: usize = SLOT_MEM;
const VBIOS: usize = SLOT_IRQ;

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let device = mind::process::args_str().trim();
    let (kind, _, engine) = cap_info(ENGINE);
    if kind != CAP_KIND_MMIO { mind::println!("[DISPLAY] NO REGISTERS GRANTED"); idle(); }
    match (device_config(ENGINE, 0), device_config(ENGINE, 4), device_config(ENGINE, 0x24)) {
        (Ok(id), Ok(command), Ok(bar)) => mind::println!("[DISPLAY] PCI {:04X}:{:04X} (DEVICE {}), COMMAND {:04X}, BAR 5 AT {:08X}: {} KIB OF ITS REGISTERS GRANTED",
                                                       id & 0xFFFF, id >> 16, device, command & 0xFFFF, bar & !0xF, engine >> 10),
        _ => mind::println!("[DISPLAY] PCI CONFIG UNAVAILABLE"),
    }
    let size = |slot: usize, kind: usize| { let (k, _, len) = cap_info(slot); (k == kind).then_some(len) };
    match size(MAILBOX, CAP_KIND_MMIO) { Some(len) => mind::println!("[DISPLAY] THE SMU MAILBOX: {} BYTES GRANTED", len), None => mind::println!("[DISPLAY] NO SMU MAILBOX GRANTED") }
    match size(SECOND_SCREEN, CAP_KIND_MMIO) { Some(len) => mind::println!("[DISPLAY] BAR 0 FOR A SECOND SCREEN: {} KIB GRANTED", len >> 10), None => mind::println!("[DISPLAY] NO SECOND SCREEN'S MEMORY GRANTED") }
    match size(VBIOS, CAP_KIND_MEMORY).and_then(|_| mind::mem::Mapping::new(VBIOS).ok()) {
        Some(table) => { let length = table.as_slice().get(4..8).map_or(0, |l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize); vbios(&table.as_slice()[..length.min(table.len())]) }
        None => mind::println!("[DISPLAY] NO VBIOS IMAGE GRANTED"),
    }
    mind::println!("[DISPLAY] NO REGISTER READ OR WRITTEN");
    idle();
}

// The VFCT's first image (AMD's layout, as Linux's amdgpu_bios.c reads it): its PCI identity, length and ROM signature.
fn vbios(table: &[u8]) {
    let u16_at = |at: usize| table.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| table.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let image = u32_at(52).map(|offset| offset as usize);
    match image.and_then(|at| Some((u16_at(at + 12)?, u16_at(at + 14)?, u32_at(at + 24)? as usize, at + 28))) {
        Some((vendor, id, length, at)) if table.starts_with(b"VFCT") => {
            let rom = table.get(at..at + 2) == Some(&[0x55, 0xAA][..]);
            mind::println!("[DISPLAY] VBIOS: VFCT {} BYTES, AN IMAGE FOR {:04X}:{:04X} OF {} BYTES, ROM SIGNATURE {}",
                           table.len(), vendor, id, length, if rom { "55AA" } else { "MISSING" });
        }
        _ => mind::println!("[DISPLAY] VBIOS: THE GRANTED TABLE IS NOT A VFCT ONE CAN READ"),
    }
}

// The grant held, nothing done with it.
fn idle() -> ! { loop { mind::time::sleep(3_600_000); } }
