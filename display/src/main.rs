#![no_std]
#![no_main]
// display (213-DRV-0086, DRV's): the Ryzen's Radeon display engine. This stub, KRN's (213-KRN-0270), shows the grant init
// gives: the registers' BAR and the device's PCI identity and command register; it reads no register and writes nothing.
use mind::abi::{BootInfo, CAP_KIND_MMIO, SLOT_DEV0};
use mind::dev::{cap_info, device_config};

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (kind, _, len) = cap_info(SLOT_DEV0);
    let device = mind::process::args_str().trim();
    if kind != CAP_KIND_MMIO { mind::println!("[DISPLAY] NO REGISTERS GRANTED"); idle(); }
    match (device_config(SLOT_DEV0, 0), device_config(SLOT_DEV0, 4), device_config(SLOT_DEV0, 0x24)) {
        (Ok(id), Ok(command), Ok(bar)) => mind::println!("[DISPLAY] PCI {:04X}:{:04X} (DEVICE {}), COMMAND {:04X}, BAR 5 AT {:08X}, {} KIB GRANTED; NO REGISTER READ OR WRITTEN",
                                                       id & 0xFFFF, id >> 16, device, command & 0xFFFF, bar & !0xF, len >> 10),
        _ => mind::println!("[DISPLAY] PCI CONFIG UNAVAILABLE"),
    }
    idle();
}

// The grant held, nothing done with it.
fn idle() -> ! { loop { mind::time::sleep(3_600_000); } }
