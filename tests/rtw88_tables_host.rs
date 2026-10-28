//! rtw88_wifi's tables and their conditions (550-DRV-0042): the selection follows rtw88's phy.c, and the generated
//! tables are the pinned source's.
#[path = "../rtw88_wifi/src/cond.rs"]
mod cond;
#[path = "../rtw88_wifi/src/tables.rs"]
#[allow(dead_code)]
mod tables;
#[path = "../rtw88_wifi/src/load.rs"]
#[allow(dead_code)]
mod load;

use cond::{holds, select, Chip};

fn picked(table: &[(u32, u32)], chip: &Chip) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    select::<()>(table, chip, |a, v| { out.push((a, v)); Ok(()) }).unwrap();
    out
}

#[test]
fn an_if_elif_else_chain_takes_one_branch() {
    // if rfe 1 { A } elif rfe 2 { B } else { C } endif, then D.
    let table = [
        (0x8000_0001, 0), (0x4000_0000, 0), (0x10, 1),
        (0x9000_0002, 0), (0x4000_0000, 0), (0x10, 2),
        (0xA000_0000, 0), (0x10, 3),
        (0xB000_0000, 0), (0x20, 4),
    ];
    assert_eq!(picked(&table, &Chip::pcie(0, 0, 1)), vec![(0x10, 1), (0x20, 4)]);
    assert_eq!(picked(&table, &Chip::pcie(0, 0, 2)), vec![(0x10, 2), (0x20, 4)]);
    assert_eq!(picked(&table, &Chip::pcie(0, 0, 7)), vec![(0x10, 3), (0x20, 4)]);
}

#[test]
fn a_condition_names_cut_package_and_interface_or_leaves_them_open() {
    let chip = Chip::pcie(2, 0, 4);
    assert_eq!(chip, Chip { cut: 2, pkg: 15, intf: 1, rfe: 4 });
    assert!(holds(0x8000_0004, &chip));            // rfe 4, the rest open
    assert!(holds(0x8200_0104, &chip));            // cut 2, PCIe, rfe 4
    assert!(!holds(0x8300_0004, &chip));           // cut 3
    assert!(!holds(0x8000_0204, &chip));           // USB
    assert!(!holds(0x8000_0005, &chip));           // rfe 5
    assert!(!holds(0x8000_1004, &chip));           // package 1
}

#[test]
fn the_generated_tables_are_whole() {
    assert_eq!((tables::MAC.len(), tables::BB.len(), tables::AGC.len(), tables::AGC_BTG_TYPE2.len(), tables::RF_A.len()),
               (138, 1680, 1200, 390, 2712));
    assert_eq!(tables::MAC[0], (0x10, 0x43));
    assert_eq!(tables::RF_A[0], (0x8000_1005, 0));
    assert_eq!((tables::BB_PG_TYPE0.len(), tables::TXPWR_LMT_TYPE0.len()), (15, 1488));
    // Every condition opened is closed, in every table.
    for table in [&tables::MAC[..], &tables::BB, &tables::AGC, &tables::AGC_BTG_TYPE2, &tables::RF_A] {
        let opened = table.iter().filter(|(w, _)| w & 0x8000_0000 != 0 && w >> 28 & 3 == 0).count();
        let closed = table.iter().filter(|(w, _)| w & 0x8000_0000 != 0 && w >> 28 & 3 == 3).count();
        assert_eq!(opened, closed);
    }
}

struct Record { writes: Vec<(char, u32, u32)>, waited: u64 }

impl load::Io for Record {
    type Error = ();
    fn write8(&mut self, at: u32, value: u8) -> Result<(), ()> { self.writes.push(('m', at, value as u32)); Ok(()) }
    fn write32(&mut self, at: u32, value: u32) -> Result<(), ()> { self.writes.push(('w', at, value)); Ok(()) }
    fn write_rf(&mut self, at: u32, value: u32) -> Result<(), ()> { self.writes.push(('r', at, value)); Ok(()) }
    fn delay_us(&mut self, us: u32) -> Result<(), ()> { self.waited += us as u64; Ok(()) }
}

#[test]
fn loading_writes_bytes_words_and_rf_with_their_delays() {
    let mut io = Record { writes: Vec::new(), waited: 0 };
    load::all(&mut io, &Chip::pcie(0, 0, 0), false).unwrap();
    assert_eq!(io.writes[0], ('m', 0x10, 0x43));
    assert!(io.writes.iter().all(|&(k, a, _)| k != 'w' || !(0xf9..=0xfe).contains(&a)), "a BB delay written as a register");
    assert!(io.writes.iter().all(|&(k, a, _)| k != 'r' || (a != 0xffe && a != 0xfe)), "an RF delay written as a register");
    assert!(io.writes.iter().any(|&(k, _, _)| k == 'r'));
    assert!(io.waited > 0);
}
