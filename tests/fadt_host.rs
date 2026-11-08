//! Host tests of the FADT's fixed hardware (common/fadt.rs, 000-KRN-0216): the blocks the kernel may lend, the 32-bit
//! fields and the generic addresses that supersede them, the power button's kind, and what an untrusted table cannot
//! make the parser accept.
#[path = "../common/fadt.rs"]
mod fadt;

use fadt::{parse, Block, Fixed, PWRBTN};

// A revision-6 FADT as QEMU's q35 builds it: SCI 9, SMI_CMD 0xB2 with ACPI_ENABLE 2, PM1a events at 0x600 (4 bytes),
// PM1a control at 0x604 (2), GPE0 at 0x620 (16), flags with PWR_BUTTON clear; no generic addresses unless set.
fn table() -> Vec<u8> {
    let mut t = vec![0u8; 276];
    t[..4].copy_from_slice(b"FACP");
    t[4..8].copy_from_slice(&276u32.to_le_bytes());
    t[8] = 6;
    t[46..48].copy_from_slice(&9u16.to_le_bytes());
    t[48..52].copy_from_slice(&0xB2u32.to_le_bytes());
    t[52] = 2;
    t[56..60].copy_from_slice(&0x600u32.to_le_bytes());
    t[64..68].copy_from_slice(&0x604u32.to_le_bytes());
    t[80..84].copy_from_slice(&0x620u32.to_le_bytes());
    t[88] = 4;
    t[89] = 2;
    t[92] = 16;
    t[112..116].copy_from_slice(&0x0000_84A5u32.to_le_bytes());
    t
}

fn gas(t: &mut [u8], at: usize, space: u8, bits: u8, address: u64) {
    t[at] = space;
    t[at + 1] = bits;
    t[at + 4..at + 12].copy_from_slice(&address.to_le_bytes());
}

#[test]
fn qemus_table() {
    let f = parse(&table()).unwrap();
    assert_eq!((f.sci, f.acpi_enable), (9, 2));
    assert_eq!(f.smi_cmd, Some(Block { port: 0xB2, len: 1 }));
    assert_eq!(f.pm1a_evt, Some(Block { port: 0x600, len: 4 }));
    assert_eq!(f.pm1a_cnt, Some(Block { port: 0x604, len: 2 }));
    assert_eq!(f.gpe0, Some(Block { port: 0x620, len: 16 }));
    assert_eq!((f.pm1b_evt, f.pm1b_cnt, f.gpe1), (None, None, None));
    assert!(f.fixed_power_button() && !f.hardware_reduced());
    assert_eq!(Fixed::status_enable(f.pm1a_evt.unwrap()), (0x600, 0x602));
    assert_eq!(f.blocks().iter().flatten().count(), 4);
    assert_eq!(PWRBTN, 0x100);
}

#[test]
fn the_dexps_flags_name_a_fixed_button() {
    let mut t = table();
    t[112..116].copy_from_slice(&0x3C4E5u32.to_le_bytes()); // hw0008.txt of 2026-11-08
    assert!(parse(&t).unwrap().fixed_power_button());
    t[112..116].copy_from_slice(&(0x3C4E5u32 | 1 << 4).to_le_bytes());
    assert!(!parse(&t).unwrap().fixed_power_button());
    t[112..116].copy_from_slice(&(1u32 << 20).to_le_bytes());
    assert!(parse(&t).unwrap().hardware_reduced());
}

#[test]
fn a_generic_address_supersedes_the_32_bit_field() {
    let mut t = table();
    gas(&mut t, 148, 1, 32, 0x1800); // X_PM1a_EVT_BLK in system I/O
    gas(&mut t, 172, 1, 16, 0x1804);
    assert_eq!(parse(&t).unwrap().pm1a_evt, Some(Block { port: 0x1800, len: 4 }));
    assert_eq!(parse(&t).unwrap().pm1a_cnt, Some(Block { port: 0x1804, len: 2 }));
    // No length byte: the generic address's width gives it.
    t[88] = 0;
    assert_eq!(parse(&t).unwrap().pm1a_evt, Some(Block { port: 0x1800, len: 4 }));
    // In memory space: no I/O block, even though the 32-bit field names one.
    t[88] = 4;
    gas(&mut t, 148, 0, 32, 0xFED0_0000);
    assert_eq!(parse(&t).unwrap().pm1a_evt, None);
    // Zero: the 32-bit field again.
    gas(&mut t, 148, 0, 0, 0);
    assert_eq!(parse(&t).unwrap().pm1a_evt, Some(Block { port: 0x600, len: 4 }));
}

#[test]
fn what_an_untrusted_table_cannot_make_it_accept() {
    assert!(parse(b"APIC").is_none() && parse(&table()[..115]).is_none());
    let mut wrong = table();
    wrong[..4].copy_from_slice(b"FACQ");
    assert!(parse(&wrong).is_none());
    // A block past port 0xFFFF, an odd or short event block, a GPE block of odd length: none of them.
    let mut t = table();
    t[56..60].copy_from_slice(&0xFFFEu32.to_le_bytes());
    assert_eq!(parse(&t).unwrap().pm1a_evt, None);
    let mut t = table();
    t[88] = 3;
    assert_eq!(parse(&t).unwrap().pm1a_evt, None);
    t[88] = 2;
    assert_eq!(parse(&t).unwrap().pm1a_evt, None);
    let mut t = table();
    t[92] = 5;
    assert_eq!(parse(&t).unwrap().gpe0, None);
    // SMI_CMD past 16 bits, and none.
    let mut t = table();
    t[48..52].copy_from_slice(&0x1_0000u32.to_le_bytes());
    assert_eq!(parse(&t).unwrap().smi_cmd, None);
    t[48..52].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(parse(&t).unwrap().smi_cmd, None);
    // A revision-1 table (116 bytes): the 32-bit fields alone.
    let f = parse(&table()[..116]).unwrap();
    assert_eq!(f.pm1a_evt, Some(Block { port: 0x600, len: 4 }));
}

#[test]
fn a_block_holds_only_its_own_ports() {
    let b = Block { port: 0x600, len: 4 };
    assert!(b.holds(0x600, 0x604) && b.holds(0x602, 0x604));
    assert!(!b.holds(0x5FF, 0x601) && !b.holds(0x602, 0x605) && !b.holds(0x604, 0x604));
}
