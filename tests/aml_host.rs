//! Host tests of the pin controller scan of ACPI definition blocks (kernel/src/arch/aarch64/aml.rs, issue 206). The
//! blocks are built as the ASL compiler encodes them: QEMU's PL061 device (acpi_dsdt_add_gpio: a static _CRS with
//! Memory32Fixed), the Raspberry Pi 4 EDK2 firmware's GPIO device (its _CRS a method), a QWord window, and blocks the
//! scan must not misread.
#[path = "../kernel/src/arch/aarch64/aml.rs"]
mod aml;
use aml::*;

fn name_string(name: &[u8; 4], value: &str) -> Vec<u8> { let mut v = vec![0x08]; v.extend_from_slice(name); v.push(0x0D); v.extend_from_slice(value.as_bytes()); v.push(0); v }
// PkgLength of a package whose contents are `n` bytes (the length counts itself).
fn package(n: usize) -> Vec<u8> {
    if n + 1 < 64 { vec![(n + 1) as u8] } else { let total = n + 2; vec![0x40 | (total & 0x0F) as u8, (total >> 4) as u8] }
}
fn buffer_name(name: &[u8; 4], resources: &[u8]) -> Vec<u8> {
    let mut body = vec![0x0A, resources.len() as u8]; body.extend_from_slice(resources);
    let mut v = vec![0x08]; v.extend_from_slice(name); v.push(0x11); v.extend(package(body.len())); v.extend(body); v
}
fn device(name: &[u8; 4], contents: &[u8]) -> Vec<u8> { let mut v = vec![0x5B, 0x82]; v.extend(package(4 + contents.len())); v.extend_from_slice(name); v.extend_from_slice(contents); v }
fn memory32(base: u32, size: u32) -> Vec<u8> { let mut v = vec![0x86, 9, 0, 1]; v.extend(base.to_le_bytes()); v.extend(size.to_le_bytes()); v }
fn interrupt(gsiv: u32) -> Vec<u8> { let mut v = vec![0x89, 6, 0, 0x01, 1]; v.extend(gsiv.to_le_bytes()); v }
const END: [u8; 2] = [0x79, 0];

fn scan(aml: &[u8]) -> Vec<(Pins, Option<(u64, u64)>)> { let mut out = Vec::new(); pin_controllers(aml, |k, w| out.push((k, w))); out }

#[test]
fn qemus_pl061_with_a_static_window() {
    let resources = [memory32(0x0903_0000, 0x1000), interrupt(39), END.to_vec()].concat();
    let gpio = device(b"GPO0", &[name_string(b"_HID", "ARMH0061"), name_string(b"_UID", "0"), buffer_name(b"_CRS", &resources)].concat());
    let uart = device(b"COM0", &[name_string(b"_HID", "ARMH0011"), buffer_name(b"_CRS", &[memory32(0x0900_0000, 0x1000), END.to_vec()].concat())].concat());
    assert_eq!(scan(&[uart, gpio].concat()), [(Pins::Pl061, Some((0x0903_0000, 0x1000)))]);
}

#[test]
fn the_raspberry_pi_4s_gpio_without_a_static_window() {
    // Name (RBUF, ResourceTemplate () { Memory32Fixed (ReadWrite, 0, 0xB4, RMEM) ... }) and Method (_CRS) patching it.
    let rbuf = buffer_name(b"RBUF", &[memory32(0, 0xB4), interrupt(145), END.to_vec()].concat());
    let method = [0x14, 0x0B, b'_', b'C', b'R', b'S', 0x08, 0xA4, b'R', b'B', b'U', b'F', 0x00, 0x00].to_vec(); // Method (_CRS) { Return (RBUF) }
    let gpio = device(b"GPI0", &[name_string(b"_HID", "BCM2845"), name_string(b"_CID", "BCM2845"), rbuf, method].concat());
    assert_eq!(scan(&gpio), [(Pins::Bcm2711, None)]);
}

#[test]
fn a_qword_window_above_4_gib() {
    let mut qword = vec![0x8A, 43, 0, 0, 0x0C, 0x01];
    for value in [0u64, 0x10_0000_0000, 0x10_0000_0FFF, 0, 0x1000] { qword.extend(value.to_le_bytes()); }
    let gpio = device(b"GPO1", &[name_string(b"_HID", "ARMH0061"), buffer_name(b"_CRS", &[qword, END.to_vec()].concat())].concat());
    assert_eq!(scan(&gpio), [(Pins::Pl061, Some((0x10_0000_0000, 0x1000)))]);
}

#[test]
fn another_devices_crs_is_not_taken() {
    // A PL061 without a _CRS of its own, followed by a device that has one: no window.
    let gpio = device(b"GPO0", &name_string(b"_HID", "ARMH0061"));
    let other = device(b"OTHR", &[name_string(b"_HID", "ABCD0001"), buffer_name(b"_CRS", &[memory32(0x1000, 0x1000), END.to_vec()].concat())].concat());
    assert_eq!(scan(&[gpio, other].concat()), [(Pins::Pl061, None)]);
}

#[test]
fn unknown_ids_truncated_and_garbage_blocks_give_nothing() {
    assert!(scan(&device(b"GPOX", &name_string(b"_HID", "ARMH0062"))).is_empty());
    assert!(scan(&name_string(b"_HID", "ARMH006")).is_empty());
    let full = device(b"GPO0", &[name_string(b"_HID", "ARMH0061"), buffer_name(b"_CRS", &[memory32(0x0903_0000, 0x1000), END.to_vec()].concat())].concat());
    for cut in 0..full.len() { let _ = scan(&full[..cut]); } // every truncation: no panic
    let noise: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    let _ = scan(&noise);
}
