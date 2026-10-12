//! Host tests of the Smart Battery System behind an ACPI embedded controller (libmind/src/battery.rs, 211-PRT-0008): the
//! controller and its SMBus host found in the tables' bytes, and a model of the controller (its byte protocol, the SMBus
//! host in its space, a battery, a manager and a charger behind it) read as the `battery` service reads the MacBook Pro's.
#![allow(dead_code)]
#[path = "../libmind/src/battery.rs"]
mod battery;

use battery::*;
use std::collections::HashMap;

const BASE: u8 = 0x20; // the MacBook Pro's: `_EC` 0x2010

#[derive(Clone, Copy, Default, PartialEq)]
enum Step { #[default] Idle, ReadAddress, WriteAddress, WriteValue(u8) }

// The controller: its space, the step of the protocol, the data register, and the devices on its SMBus.
#[derive(Default)]
struct Model {
    space: Vec<u8>,
    step: Step,
    full: bool,
    out: u8,
    words: HashMap<(u8, u8), u16>,
    blocks: HashMap<(u8, u8), Vec<u8>>,
    // Reads of the protocol register that still show a transaction running.
    running: usize,
    // A controller that never takes a byte (IBF stays set).
    deaf: bool,
    pauses: usize,
    transactions: usize,
}

impl Model {
    fn new() -> Self { Self { space: vec![0; 256], ..Default::default() } }
    fn read_space(&mut self, at: u8) -> u8 {
        if at == BASE && self.space[BASE as usize] != 0 {
            if self.running == 0 { self.space[BASE as usize] = 0; } else { self.running -= 1; }
        }
        self.space[at as usize]
    }
    // The SMBus host runs the transaction the protocol register names.
    fn transact(&mut self, protocol: u8) {
        self.transactions += 1;
        let base = BASE as usize;
        let (address, command) = (self.space[base + 2] >> 1, self.space[base + 3]);
        let status = match protocol {
            0x09 => match self.words.get(&(address, command)) {
                Some(&word) => { self.space[base + 4..base + 6].copy_from_slice(&word.to_le_bytes()); 0 }
                None => 0x10,
            },
            0x0B => match self.blocks.get(&(address, command)).cloned() {
                Some(bytes) => { self.space[base + 4..base + 4 + bytes.len()].copy_from_slice(&bytes); self.space[base + 0x24] = bytes.len() as u8; 0 }
                None => 0x10,
            },
            _ => 0x19,
        };
        self.space[base + 1] = 0x80 | status;
    }
}

impl Ports for Model {
    fn status(&mut self) -> u8 { self.full as u8 | if self.deaf { 2 } else { 0 } }
    fn command(&mut self, value: u8) { self.step = match value { 0x80 => Step::ReadAddress, 0x81 => Step::WriteAddress, _ => Step::Idle }; }
    fn read_data(&mut self) -> u8 { self.full = false; self.out }
    fn write_data(&mut self, value: u8) {
        match self.step {
            Step::ReadAddress => { self.out = self.read_space(value); self.full = true; self.step = Step::Idle; }
            Step::WriteAddress => self.step = Step::WriteValue(value),
            Step::WriteValue(at) => {
                self.space[at as usize] = value;
                self.step = Step::Idle;
                if at == BASE && value != 0 { self.transact(value); }
            }
            Step::Idle => {}
        }
    }
    fn pause(&mut self) { self.pauses += 1; }
}

// A MacBook Pro's battery on battery power: 87 %, 1.2 A drawn, 2 h 41 min left, capacities in tens of mWh.
fn macbook() -> Model {
    let mut model = Model::new();
    for (command, word) in [(0x16, 0x00C0), (0x0D, 87), (0x12, 161), (0x13, 0xFFFF), (0x09, 12150), (0x0A, (-1200i16) as u16), (0x0F, 5000),
                            (0x10, 5770), (0x18, 7700), (0x03, 0xE001), (0x17, 312), (0x19, 10950)] {
        model.words.insert((BATTERY, command), word);
    }
    model.words.insert((MANAGER, 0x01), 0x0001);
    model.words.insert((CHARGER, 0x13), 0xFFFF); // as MacBooks answer: no charger to the specification
    model.blocks.insert((BATTERY, MANUFACTURER), b"SMP".to_vec());
    model.blocks.insert((BATTERY, DEVICE_NAME), b"bq20z451".to_vec());
    model
}

fn host(model: Model) -> Smbus<Model> { Smbus { ec: Ec { ports: model }, base: BASE } }

#[test]
fn the_controller_and_its_smbus_host_are_found_in_the_tables() {
    // An ECDT: command/status at 0x66, data at 0x62, both in system I/O space.
    let mut ecdt = vec![0u8; 83];
    ecdt[..4].copy_from_slice(b"ECDT");
    ecdt[36] = 1; ecdt[40] = 0x66;
    ecdt[48] = 1; ecdt[52] = 0x62;
    assert_eq!(ecdt_ports(&ecdt), Some((0x66, 0x62)));
    ecdt[48] = 0; // data in memory space: not one this service can use
    assert_eq!(ecdt_ports(&ecdt), None);
    assert_eq!(ecdt_ports(b"FACP"), None);
    // The MacBook Pro's DSDT: Device (SMB0) { Name (_HID, "ACPI0001") Name (_EC, 0x2010) ... }.
    let mut aml = vec![0x5B, 0x82, 0x40, 0x10, b'S', b'M', b'B', b'0', 0x08, b'_', b'H', b'I', b'D', 0x0D];
    aml.extend_from_slice(b"ACPI0001\0");
    aml.extend_from_slice(&[0x08, b'_', b'E', b'C', b'_', 0x0B, 0x10, 0x20]);
    assert_eq!(smbus_base(&aml), Some(0x20));
    // A byte constant: the host at 0 of the controller's space.
    let byte = [&aml[..23], &[0x08, b'_', b'E', b'C', b'_', 0x0A, 0x10][..]].concat();
    assert_eq!(smbus_base(&byte), Some(0));
    // `_EC` a method, not a constant; no ACPI0001 at all.
    let method = [&aml[..23], &[0x14, 0x08, b'_', b'E', b'C', b'_', 0x00][..]].concat();
    assert_eq!(smbus_base(&method), None);
    assert_eq!(smbus_base(b"\x0DPNP0C0A\0\x08_EC_\x0B\x10\x20"), None);
}

#[test]
fn a_macbook_battery_on_its_own_power() {
    let mut bus = host(macbook());
    assert_eq!(fitted(&mut bus), Some(true));
    assert_eq!(charger(&mut bus), None, "a charger answering 0xFFFF is none");
    let reading = read(&mut bus, false).expect("reading");
    let state = reading.state();
    assert!(!state.on_ac && !state.ac_from_charger && state.discharging && !state.charging && !state.full);
    assert_eq!((state.percent, state.to_empty, state.to_full), (87, 161, 0xFFFF));
    assert_eq!((state.millivolts, state.milliamps, state.cycles), (12150, 1200, 312));
    assert_eq!((state.remaining, state.full_capacity, state.design, state.tens_of_mwh), (5000, 5770, 7700, true));
    let mut name = [0u8; 32];
    let n = bus.read_block(BATTERY, DEVICE_NAME, &mut name).expect("name");
    assert_eq!(&name[..n], b"bq20z451");
}

#[test]
fn on_the_mains_charging_then_full() {
    let mut model = macbook();
    model.words.insert((BATTERY, 0x16), 0x0080); // not discharging
    model.words.insert((BATTERY, 0x0A), 2000);
    model.words.insert((BATTERY, 0x13), 45);
    model.words.insert((BATTERY, 0x12), 0xFFFF);
    let mut bus = host(model);
    let state = read(&mut bus, false).expect("reading").state();
    assert!(state.on_ac && state.charging && !state.discharging && !state.ac_from_charger);
    assert_eq!((state.to_full, state.to_empty, state.milliamps), (45, 0xFFFF, 2000));
    bus.ec.ports.words.insert((BATTERY, 0x16), 0x00A0); // fully charged
    bus.ec.ports.words.insert((BATTERY, 0x0A), 0);
    let state = read(&mut bus, false).expect("reading").state();
    assert!(state.on_ac && state.full && !state.charging);
}

#[test]
fn a_charger_to_the_specification_says_where_the_power_comes_from() {
    let mut model = macbook();
    model.words.insert((CHARGER, 0x13), 0x8010); // AC_PRESENT, and bit 4 as the specification asks
    let mut bus = host(model);
    let with_charger = charger(&mut bus).is_some();
    assert!(with_charger);
    let state = read(&mut bus, with_charger).expect("reading").state();
    assert!(state.on_ac && state.ac_from_charger && state.discharging, "the charger decides, though the battery discharges");
    assert!(!charger_answers(0x8000) && !charger_answers(0xFFFF) && charger_answers(0x0010));
}

#[test]
fn a_transaction_waits_for_the_firmwares_and_for_its_own() {
    let mut model = macbook();
    model.space[BASE as usize] = 0x09; // the firmware's transaction runs
    model.running = 3;
    let mut bus = host(model);
    assert_eq!(bus.read_word(BATTERY, 0x0D), Ok(87));
    assert_eq!(bus.ec.ports.pauses, 3, "a pause for each read that found the host busy");
    bus.ec.ports.running = 5; // ours takes a while
    bus.ec.ports.pauses = 0;
    bus.ec.ports.space[BASE as usize] = 0;
    assert_eq!(bus.read_word(BATTERY, 0x0D), Ok(87));
    assert_eq!(bus.ec.ports.pauses, 5);
}

#[test]
fn failures_are_told_apart() {
    let mut bus = host(Model::new());
    assert_eq!(bus.read_word(BATTERY, 0x0D), Err(SmbusError::Status(0x10)), "no device at the address");
    assert_eq!(fitted(&mut bus), None, "no manager");
    let mut stuck = Model::new();
    stuck.space[BASE as usize] = 0x09;
    stuck.running = usize::MAX;
    let mut bus = host(stuck);
    assert_eq!(bus.read_word(BATTERY, 0x0D), Err(SmbusError::Busy));
    assert_eq!(bus.ec.ports.transactions, 0, "nothing started over the firmware's transaction");
    let mut deaf = Model::new();
    deaf.deaf = true;
    let mut bus = host(deaf);
    assert_eq!(bus.read_word(BATTERY, 0x0D), Err(SmbusError::Controller));
    assert!(bus.ec.ports.pauses >= 100, "it waited before giving up");
}

#[test]
fn minutes_are_hours_and_minutes() {
    let mut out = String::new();
    write_minutes(&mut out, 161).unwrap();
    out.push(' ');
    write_minutes(&mut out, 0xFFFF).unwrap();
    out.push(' ');
    write_minutes(&mut out, 5).unwrap();
    assert_eq!(out, "2:41 - 0:05");
}
