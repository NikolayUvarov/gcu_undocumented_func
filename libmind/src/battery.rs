//! The Smart Battery System behind an ACPI embedded controller (211-PRT-0008): where the controller and its SMBus host
//! are in the ACPI tables, the controller's byte protocol (ACPI 6.5, 12.3), the SMBus host interface in its space (12.9),
//! and the readings of the battery, the manager and the charger (Smart Battery Data 1.1, Smart Battery System Manager
//! 1.1). The `battery` service runs it on the controller's two ports; tests/battery_host.rs on a model of them.

/// The ports of the embedded controller's command/status and data registers, from an ECDT (ACPI 6.5, 5.2.16): both in
/// system I/O space.
pub fn ecdt_ports(table: &[u8]) -> Option<(u16, u16)> {
    if table.get(..4)? != b"ECDT" { return None; }
    let port = |at: usize| -> Option<u16> {
        if *table.get(at)? != 1 { return None; }
        let address = u64::from_le_bytes(table.get(at + 4..at + 12)?.try_into().ok()?);
        u16::try_from(address).ok().filter(|&p| p != 0)
    };
    Some((port(36)?, port(48)?))
}

/// The SMBus host controller's base in the controller's space: the high byte of `_EC` in the device whose `_HID` is
/// "ACPI0001", found in a DSDT or SSDT's AML as bytes (the string, then within 128 bytes the name `_EC_` and a byte,
/// word or dword constant), as no AML interpreter runs. None: no such device, or an `_EC` that is not a constant.
pub fn smbus_base(aml: &[u8]) -> Option<u8> {
    const HID: &[u8] = b"\x0DACPI0001\x00";
    let at = aml.windows(HID.len()).position(|w| w == HID)? + HID.len();
    let near = &aml[at..aml.len().min(at + 128)];
    let name = near.windows(5).position(|w| w == b"\x08_EC_")? + 5;
    let value = match *near.get(name)? {
        0x0A => *near.get(name + 1)? as u32,
        0x0B => u16::from_le_bytes([*near.get(name + 1)?, *near.get(name + 2)?]) as u32,
        0x0C => u32::from_le_bytes(near.get(name + 1..name + 5)?.try_into().ok()?),
        _ => return None,
    };
    Some((value >> 8) as u8)
}

/// The controller's two ports, and a short wait.
pub trait Ports {
    /// The status register (EC_SC read).
    fn status(&mut self) -> u8;
    /// The command register (EC_SC write).
    fn command(&mut self, value: u8);
    fn read_data(&mut self) -> u8;
    fn write_data(&mut self, value: u8);
    /// About a millisecond.
    fn pause(&mut self);
}

const OBF: u8 = 1; // a byte waits in the data register
const IBF: u8 = 2; // the controller has not taken the last byte written
const RD_EC: u8 = 0x80;
const WR_EC: u8 = 0x81;
const SPINS: usize = 100; // status reads before each pause
const STEP_MS: usize = 100; // a step of the protocol's limit

/// The embedded controller's space, a byte at a time.
pub struct Ec<P: Ports> { pub ports: P }

impl<P: Ports> Ec<P> {
    fn wait(&mut self, ready: impl Fn(u8) -> bool) -> Option<()> {
        for _ in 0..STEP_MS {
            for _ in 0..SPINS { if ready(self.ports.status()) { return Some(()); } }
            self.ports.pause();
        }
        None
    }

    /// The byte at `address` (RD_EC); None when the controller does not answer in time.
    pub fn read(&mut self, address: u8) -> Option<u8> {
        self.wait(|s| s & IBF == 0)?;
        if self.ports.status() & OBF != 0 { let _ = self.ports.read_data(); } // a byte nobody took
        self.ports.command(RD_EC);
        self.wait(|s| s & IBF == 0)?;
        self.ports.write_data(address);
        self.wait(|s| s & OBF != 0)?;
        Some(self.ports.read_data())
    }

    /// Writes `value` at `address` (WR_EC).
    pub fn write(&mut self, address: u8, value: u8) -> Option<()> {
        self.wait(|s| s & IBF == 0)?;
        self.ports.command(WR_EC);
        self.wait(|s| s & IBF == 0)?;
        self.ports.write_data(address);
        self.wait(|s| s & IBF == 0)?;
        self.ports.write_data(value);
        self.wait(|s| s & IBF == 0)
    }
}

// The SMBus host interface's registers from its base (ACPI 6.5, 12.9.1), and its protocols.
const PRTCL: u8 = 0;
const STS: u8 = 1;
const ADDR: u8 = 2;
const CMD: u8 = 3;
const DATA: u8 = 4;
const BCNT: u8 = 0x24;
const READ_WORD: u8 = 0x09;
const READ_BLOCK: u8 = 0x0B;
const DONE_MS: usize = 1000; // as long as the firmware's own methods wait for a transaction

/// Why an SMBus read failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmbusError {
    /// The embedded controller did not answer.
    Controller,
    /// The host stayed busy (the firmware's transaction, or ours that did not end).
    Busy,
    /// The host's status code (ACPI 6.5, 12.9.1.2), such as 0x10: no device acknowledged the address.
    Status(u8),
}

/// The SMBus host controller at `base` in the controller's space.
pub struct Smbus<P: Ports> { pub ec: Ec<P>, pub base: u8 }

impl<P: Ports> Smbus<P> {
    fn register(&mut self, offset: u8) -> Result<u8, SmbusError> { self.ec.read(self.base.wrapping_add(offset)).ok_or(SmbusError::Controller) }
    fn set(&mut self, offset: u8, value: u8) -> Result<(), SmbusError> { self.ec.write(self.base.wrapping_add(offset), value).ok_or(SmbusError::Controller) }

    // Waits until the protocol register reads 0: the host is free, or a transaction it ran has ended.
    fn idle(&mut self, limit_ms: usize) -> Result<(), SmbusError> {
        for _ in 0..limit_ms {
            if self.register(PRTCL)? == 0 { return Ok(()); }
            self.ec.ports.pause();
        }
        Err(SmbusError::Busy)
    }

    // One transaction: the device's address and command, then the protocol, which starts it.
    fn run(&mut self, address: u8, command: u8, protocol: u8) -> Result<(), SmbusError> {
        self.idle(STEP_MS)?;
        self.set(ADDR, address << 1)?;
        self.set(CMD, command)?;
        self.set(PRTCL, protocol)?;
        self.idle(DONE_MS)?;
        match self.register(STS)? & 0x1F { 0 => Ok(()), code => Err(SmbusError::Status(code)) }
    }

    /// SMBus Read Word.
    pub fn read_word(&mut self, address: u8, command: u8) -> Result<u16, SmbusError> {
        self.run(address, command, READ_WORD)?;
        Ok(u16::from_le_bytes([self.register(DATA)?, self.register(DATA + 1)?]))
    }

    /// SMBus Read Block into `out`; the bytes read.
    pub fn read_block(&mut self, address: u8, command: u8, out: &mut [u8; 32]) -> Result<usize, SmbusError> {
        self.run(address, command, READ_BLOCK)?;
        let count = (self.register(BCNT)? as usize).min(out.len());
        for (i, byte) in out[..count].iter_mut().enumerate() { *byte = self.register(DATA + i as u8)?; }
        Ok(count)
    }
}

/// SMBus addresses of the Smart Battery System.
pub const BATTERY: u8 = 0x0B;
pub const MANAGER: u8 = 0x0A;
pub const CHARGER: u8 = 0x09;
// Smart Battery Data 1.1 commands.
const MODE: u8 = 0x03;
const VOLTAGE: u8 = 0x09;
const CURRENT: u8 = 0x0A;
const RELATIVE_CHARGE: u8 = 0x0D;
const REMAINING: u8 = 0x0F;
const FULL_CHARGE: u8 = 0x10;
const TO_EMPTY: u8 = 0x12;
const TO_FULL: u8 = 0x13;
const BATTERY_STATUS: u8 = 0x16;
const CYCLES: u8 = 0x17;
pub const DESIGN_CAPACITY: u8 = 0x18;
pub const DESIGN_VOLTAGE: u8 = 0x19;
pub const MANUFACTURER: u8 = 0x20;
pub const DEVICE_NAME: u8 = 0x21;
pub const CHEMISTRY: u8 = 0x22;
// The manager's BatterySystemState (bits 0-3: a battery in that slot) and the charger's ChargerStatus.
const SYSTEM_STATE: u8 = 0x01;
const CHARGER_STATUS: u8 = 0x13;
// BatteryStatus bits.
const DISCHARGING: u16 = 0x40;
const FULLY_CHARGED: u16 = 0x20;
// ChargerStatus: AC_PRESENT, and bit 4 that the specification requires set (a charger that answers 0xFFFF, as on
// MacBooks, or clears it, is not one: Linux's sbs driver ignores it the same way).
const AC_PRESENT: u16 = 0x8000;
const CHARGER_SPEC: u16 = 0x10;

/// Whether a ChargerStatus word comes from a charger that follows the specification.
pub fn charger_answers(status: u16) -> bool { status & CHARGER_SPEC != 0 && status != 0xFFFF }

/// One reading of the battery, the words as the devices gave them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub status: u16, pub percent: u16, pub to_empty: u16, pub to_full: u16, pub millivolts: u16, pub current: u16,
    pub remaining: u16, pub full: u16, pub design: u16, pub mode: u16, pub cycles: u16,
    /// ChargerStatus where the charger answers as the specification asks.
    pub charger: Option<u16>,
}

/// What a reading means (the fields of idl/battery.wit's `status` but its age).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub on_ac: bool, pub ac_from_charger: bool, pub charging: bool, pub discharging: bool, pub full: bool, pub percent: u8,
    pub to_empty: u16, pub to_full: u16, pub millivolts: u16, pub milliamps: u16, pub remaining: u16, pub full_capacity: u16,
    pub design: u16, pub tens_of_mwh: bool, pub cycles: u16,
}

impl Reading {
    pub fn state(&self) -> State {
        let current = self.current as i16;
        let discharging = self.status & DISCHARGING != 0;
        let full = self.status & FULLY_CHARGED != 0;
        State {
            on_ac: self.charger.map_or(!discharging, |c| c & AC_PRESENT != 0),
            ac_from_charger: self.charger.is_some(),
            charging: !discharging && !full && current > 0,
            discharging, full,
            percent: self.percent.min(100) as u8,
            to_empty: self.to_empty, to_full: self.to_full, millivolts: self.millivolts,
            milliamps: current.unsigned_abs(),
            remaining: self.remaining, full_capacity: self.full, design: self.design,
            tens_of_mwh: self.mode & 0x8000 != 0,
            cycles: self.cycles,
        }
    }
}

/// Whether the manager says a battery is fitted: None when there is no manager (then the battery's own answer decides).
pub fn fitted<P: Ports>(bus: &mut Smbus<P>) -> Option<bool> { bus.read_word(MANAGER, SYSTEM_STATE).ok().map(|s| s & 0xF != 0) }

/// The charger's status where it follows the specification, else None.
pub fn charger<P: Ports>(bus: &mut Smbus<P>) -> Option<u16> { bus.read_word(CHARGER, CHARGER_STATUS).ok().filter(|&s| charger_answers(s)) }

/// Reads the battery whole, a transaction a value; `with_charger`: the charger answered at start.
pub fn read<P: Ports>(bus: &mut Smbus<P>, with_charger: bool) -> Result<Reading, SmbusError> {
    let mut word = |command| bus.read_word(BATTERY, command);
    let mut reading = Reading {
        status: word(BATTERY_STATUS)?, percent: word(RELATIVE_CHARGE)?, to_empty: word(TO_EMPTY)?, to_full: word(TO_FULL)?,
        millivolts: word(VOLTAGE)?, current: word(CURRENT)?, remaining: word(REMAINING)?, full: word(FULL_CHARGE)?,
        design: word(DESIGN_CAPACITY)?, mode: word(MODE)?, cycles: word(CYCLES).unwrap_or(0), charger: None,
    };
    if with_charger { reading.charger = charger(bus); }
    Ok(reading)
}

/// Minutes as "2:41", or "-" for 65535 (not discharging, or not charging).
pub fn write_minutes(out: &mut impl core::fmt::Write, value: u16) -> core::fmt::Result {
    if value == 0xFFFF { out.write_str("-") } else { write!(out, "{}:{:02}", value / 60, value % 60) }
}
