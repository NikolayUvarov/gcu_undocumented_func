//! Host tests of the pin controller model (libmind/src/gpio.rs, issue 207): the BCM2711's and the PL061's registers
//! (models that behave as the datasheets say), who may change a pin, and the hwdocs tables of this repository.
#![allow(dead_code)]
use std::cell::RefCell;
#[path = "../libmind/src/gpio.rs"]
mod gpio;
use gpio::*;

// The BCM2711 GPIO block: GPSETn and GPCLRn drive the levels (GPLEVn) of output pins; the rest is plain storage.
struct Bcm(RefCell<[u32; 64]>);
impl Registers for Bcm {
    fn read32(&self, offset: usize) -> u32 { self.0.borrow()[offset / 4] }
    fn write32(&self, offset: usize, value: u32) {
        let mut r = self.0.borrow_mut();
        match offset {
            0x1C | 0x20 => r[(0x34 + offset - 0x1C) / 4] |= value,
            0x28 | 0x2C => r[(0x34 + offset - 0x28) / 4] &= !value,
            _ => r[offset / 4] = value,
        }
    }
}
// The PL061: DATA writes change only the pins the address bits 9:2 select, and only outputs; reads are masked too.
struct Pl061(RefCell<(u32, u32)>); // (data, direction)
impl Registers for Pl061 {
    fn read32(&self, offset: usize) -> u32 { let (data, dir) = *self.0.borrow(); if offset == 0x400 { dir } else { data & (offset as u32 >> 2) } }
    fn write32(&self, offset: usize, value: u32) {
        let mut s = self.0.borrow_mut();
        if offset == 0x400 { s.1 = value & 0xFF; } else { let mask = (offset as u32 >> 2) & s.1; s.0 = s.0 & !mask | value & mask; }
    }
}

fn bcm() -> Controller<Bcm> { Controller { kind: Kind::Bcm2711, registers: Bcm(RefCell::new([0; 64])) } }

#[test]
fn bcm2711_functions_of_every_pin() {
    let c = bcm();
    for pin in 0..58 {
        for function in 0..8 {
            c.set_function(pin, function).unwrap();
            assert_eq!(c.function(pin).unwrap(), function, "pin {pin}");
        }
        c.set_function(pin, INPUT).unwrap();
    }
    // GPIO14 as ALT0 (TXD0) is FSEL1 bits 14:12 = 100, its neighbours untouched.
    c.set_function(13, OUTPUT).unwrap(); c.set_function(15, ALT0 + 5).unwrap(); c.set_function(14, ALT0).unwrap();
    assert_eq!(c.registers.read32(0x04), 0b010_100_001 << 9);
    assert_eq!(c.set_function(58, INPUT), Err(Refusal::NoPin));
    assert_eq!(c.set_function(3, 8), Err(Refusal::Unsupported));
}

#[test]
fn bcm2711_levels_and_pulls() {
    let c = bcm();
    assert_eq!(c.write(40, true), Err(Refusal::Unsupported), "an input is not driven");
    c.set_function(40, OUTPUT).unwrap();
    c.write(40, true).unwrap();
    assert!(c.level(40).unwrap());
    assert_eq!(c.registers.read32(0x38), 1 << 8); // GPLEV1 bit 8
    c.write(40, false).unwrap();
    assert!(!c.level(40).unwrap());
    for (pin, pull) in [(0, Pull::Up), (15, Pull::Down), (16, Pull::None), (57, Pull::Up)] {
        c.set_pull(pin, pull).unwrap();
        assert_eq!(c.pull(pin).unwrap(), pull);
    }
    assert_eq!(c.registers.read32(0xE4), 1 | 2 << 30);
}

#[test]
fn pl061_directions_and_levels() {
    let c = Controller { kind: Kind::Pl061, registers: Pl061(RefCell::new((0, 0))) };
    c.set_function(3, OUTPUT).unwrap();
    c.write(3, true).unwrap();
    assert!(c.level(3).unwrap() && !c.level(2).unwrap());
    c.set_function(2, OUTPUT).unwrap();
    c.write(2, true).unwrap(); c.write(3, false).unwrap();
    assert_eq!(c.registers.0.borrow().0, 0b0100, "masked writes leave the other pins");
    assert_eq!(c.set_function(3, ALT0), Err(Refusal::Unsupported));
    assert_eq!(c.set_pull(3, Pull::Up), Err(Refusal::Unsupported));
    assert_eq!(c.level(8), Err(Refusal::NoPin));
}

#[test]
fn who_may_change_a_pin() {
    assert_eq!(may_change(0, false), Err(Refusal::Denied), "reading clients do not drive pins");
    assert_eq!(may_change(BADGE_CONTROL, false), Ok(()));
    assert_eq!(may_change(BADGE_CONTROL, true), Err(Refusal::Reserved));
    assert_eq!(may_change(BADGE_PLATFORM, true), Ok(()));
}

#[test]
fn the_hwdocs_tables_parse() {
    let pins = Pins::parse(&std::fs::read_to_string("hwdocs/socs/bcm2711.pins").unwrap()).expect("bcm2711.pins");
    assert_eq!((pins.soc.as_str(), pins.count), ("bcm2711", 58));
    assert_eq!(pins.function_name(14, ALT0), Some("TXD0"));
    assert_eq!(pins.function_name(14, ALT0 + 5), Some("TXD1"));
    assert_eq!(pins.function_name(16, ALT0), None, "reserved");
    assert_eq!(pins.function_name(35, ALT0 + 4), Some("RGMII_START_STOP"));
    assert_eq!((pins.pulls[8], pins.pulls[9], pins.pulls[28]), (Pull::Up, Pull::Down, Pull::None));
    assert!((46..58).all(|p| pins.alternates[p].iter().all(|n| n.is_empty())), "internal pins");
    let pl061 = Pins::parse(&std::fs::read_to_string("hwdocs/socs/pl061.pins").unwrap()).expect("pl061.pins");
    assert_eq!(pl061.count, 8);
    let board = Board::parse(&std::fs::read_to_string("hwdocs/boards/rpi4b.board").unwrap()).expect("rpi4b.board");
    assert_eq!((board.board.as_str(), board.soc.as_str()), ("rpi4b", "bcm2711"));
    assert_eq!((board.positions[14], board.positions[0], board.positions[21]), (8, 27, 40));
    assert!(board.reserved(0) && board.reserved(15) && board.reserved(30) && !board.reserved(17));
    assert_eq!((0..58).filter(|&p| board.positions[p] != 0).count(), 28, "the header's GPIO pins");
}

#[test]
fn malformed_tables_are_refused() {
    assert!(Pins::parse("soc x\npins 2\n5 up A\n").is_none(), "a pin beyond the count");
    assert!(Pins::parse("soc x\npins 200\n").is_none());
    assert!(Pins::parse("pins 2\n0 up\n").is_none(), "no SoC");
    assert!(Pins::parse("soc x\npins 2\n0 sideways\n").is_none());
    assert!(Board::parse("board b\nsoc s\nreserved 9-3\n").is_none());
    assert!(Board::parse("board b\nsoc s\nreserved 0-99\n").is_none());
    assert!(Board::parse("board b\nsoc s\n3 99\n").is_none());
}
