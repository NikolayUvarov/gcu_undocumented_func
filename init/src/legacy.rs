//! Boot report of legacy hardware: devices that only a superseded interface drives. Each check names the code that
//! exists only for it (docs/legacy.md, and every such place in the sources is marked `LEGACY:`); when nothing needs it
//! any more, the check and that code go together. Runs while init still holds the platform privilege.
use mind::abi::SLOT_DEV0;
use mind::dev::device_config_at;
use mind::platform::find_device_id;
use mind::virtio::Layout;

const VIRTIO: u32 = 0x1AF4;

// Indices of all devices with vendor VirtIO and a legacy or transitional device ID (0x1000..=0x103F).
fn virtio_legacy_ids(mut each: impl FnMut(usize)) {
    for id in 0x1000u32..0x1040 {
        for nth in 0.. { match find_device_id(0, 0, VIRTIO | id << 16, nth) { Ok(index) => each(index), Err(_) => break } }
    }
}
fn count(class: u32, mask: u32) -> usize { (0..).take_while(|&nth| find_device_id(class, mask, 0, nth).is_ok()).count() }

/// Prints what was found; returns how many devices need legacy support.
pub fn report() -> usize {
    let (mut only, mut transitional) = (0, 0);
    virtio_legacy_ids(|index| {
        let modern = Layout::read_with(|offset| device_config_at(SLOT_DEV0, index, offset).ok()).and_then(|l| l.single_bar()).is_some();
        if modern { transitional += 1; } else { only += 1; }
    });
    // (what, how many, the code that exists for it)
    let checks = [
        ("VIRTIO DEVICE WITH ONLY THE LEGACY INTERFACE", only, "virtio_net legacy transport"),
        ("IDE CONTROLLER", count(0x01_01_00, 0xFF_FF_00), "ata (PIO on ports 0x1F0)"),
        ("AC97 AUDIO", count(0x04_01_00, 0xFF_FF_00), "audio_gw"),
    ];
    let mut found = 0;
    for (what, n, code) in checks {
        if n > 0 { mind::println!("[INIT] LEGACY {}: FOUND {} (DRIVEN BY {})", what, n, code); found += n; } else { mind::println!("[INIT] LEGACY {}: NOT FOUND", what); }
    }
    if transitional > 0 { mind::println!("[INIT] VIRTIO TRANSITIONAL DEVICES: {} (LEGACY INTERFACE PRESENT, NOT USED)", transitional); }
    mind::println!("[INIT] LEGACY ISA DEVICES, NOT ENUMERABLE, ASSUMED BY THE PLATFORM PROFILE: PS/2 KEYBOARD, CMOS RTC, COM1, PRIMARY IDE PORTS, 8259 PIC AND PIT");
    if found > 0 { mind::println!("[INIT] LEGACY DEVICES FOUND: {} (docs/legacy.md)", found); } else { mind::println!("[INIT] NO LEGACY PCI DEVICES FOUND"); }
    found
}
