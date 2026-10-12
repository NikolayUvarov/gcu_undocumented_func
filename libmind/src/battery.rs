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

/// Where the controller's space says the mains adapter is plugged in: the byte and bit of the one-bit field the AC
/// adapter's `_PSR` returns (`_HID` "ACPI0003"), either at once or through a name a `Store` fills from that field (the
/// MacBook Pro: `Return (PWRS)`, `PWRS = EACP`, EACP at 0x60 bit 1). Found in AML bytes as `smbus_base` is; None when
/// the chain is longer or the field is not one bit of an `EmbeddedControl` region.
pub fn ac_bit(aml: &[u8]) -> Option<(u8, u8)> {
    let name = psr_name(aml)?;
    ec_bit(aml, name).or_else(|| {
        (0..aml.len()).filter(|&at| aml[at] == 0x70).find_map(|at| {
            let (source, taken) = name_string(aml, at + 1)?;
            (name_string(aml, at + 1 + taken)?.0 == name).then(|| ec_bit(aml, source)).flatten()
        })
    })
}

// AML's PkgLength at `at` (ACPI 6.5, 20.2.4): the length it encodes and the bytes it takes.
fn package_length(aml: &[u8], at: usize) -> Option<(usize, usize)> {
    let lead = *aml.get(at)?;
    let extra = (lead >> 6) as usize;
    if extra == 0 { return Some(((lead & 0x3F) as usize, 1)); }
    let mut length = (lead & 0x0F) as usize;
    for i in 0..extra { length |= (*aml.get(at + 1 + i)? as usize) << (4 + 8 * i); }
    Some((length, 1 + extra))
}

fn name_seg(aml: &[u8], at: usize) -> Option<[u8; 4]> {
    let seg: [u8; 4] = aml.get(at..at + 4)?.try_into().ok()?;
    let valid = |b: u8, first: bool| b == b'_' || b.is_ascii_uppercase() || (!first && b.is_ascii_digit());
    (valid(seg[0], true) && seg[1..].iter().all(|&b| valid(b, false))).then_some(seg)
}

// A NameString at `at`: its last segment and the bytes it takes; None for the null name.
fn name_string(aml: &[u8], at: usize) -> Option<([u8; 4], usize)> {
    let mut i = at;
    if aml.get(i) == Some(&b'\\') { i += 1 } else { while aml.get(i) == Some(&b'^') { i += 1 } }
    let count = match *aml.get(i)? {
        0x2E => { i += 1; 2 }
        0x2F => { i += 2; *aml.get(i - 1)? as usize }
        _ => 1,
    };
    let mut last = None;
    for _ in 0..count { last = Some(name_seg(aml, i)?); i += 4; }
    Some((last?, i - at))
}

// The ExtOpPrefix operations with a PkgLength (Field, Device) at `at`: the body's start and end.
fn package(aml: &[u8], at: usize, op: u8) -> Option<(usize, usize)> {
    if aml.get(at..at + 2)? != [0x5B, op] { return None; }
    let (length, taken) = package_length(aml, at + 2)?;
    Some((at + 2 + taken, (at + 2 + length).min(aml.len())))
}

// The name the AC adapter's `_PSR` returns: the first `Return` of a name in it.
fn psr_name(aml: &[u8]) -> Option<[u8; 4]> {
    const HID: &[u8] = b"\x08_HID\x0DACPI0003\x00";
    (0..aml.len()).filter_map(|at| package(aml, at, 0x82)).find_map(|(start, end)| {
        let body = &aml[start..end];
        body.windows(HID.len()).position(|w| w == HID)?;
        (0..body.len()).filter(|&at| body[at] == 0x14).find_map(|at| {
            let (length, taken) = package_length(body, at + 1)?;
            if name_seg(body, at + 1 + taken)? != *b"_PSR" { return None; }
            let method = &body[at + 1 + taken + 5..(at + 1 + length).min(body.len())];
            (0..method.len()).filter(|&i| method[i] == 0xA4).find_map(|i| name_string(method, i + 1)).map(|(name, _)| name)
        })
    })
}

// The byte and bit of the one-bit field `wanted` in an `EmbeddedControl` region's field lists.
fn ec_bit(aml: &[u8], wanted: [u8; 4]) -> Option<(u8, u8)> {
    let region = (0..aml.len()).find_map(|at| {
        if aml.get(at..at + 2)? != [0x5B, 0x80] { return None; }
        let (name, taken) = name_string(aml, at + 2)?;
        let at = at + 2 + taken;
        if *aml.get(at)? != 0x03 { return None; } // EmbeddedControl
        let offset = match *aml.get(at + 1)? { 0x00 => 0, 0x01 => 1, 0x0A => *aml.get(at + 2)? as usize, _ => return None };
        Some((name, offset))
    })?;
    (0..aml.len()).filter_map(|at| package(aml, at, 0x81)).find_map(|(start, end)| {
        let (name, taken) = name_string(aml, start)?;
        if name != region.0 { return None; }
        let (mut at, mut bit) = (start + taken + 1, 0);
        while at < end {
            match aml[at] {
                0x00 => { let (length, taken) = package_length(aml, at + 1)?; bit += length; at += 1 + taken; }
                0x01 => at += 3,
                0x03 => at += 4,
                0x02 => return None, // a ConnectField: not in a controller's space
                _ => {
                    let seg = name_seg(aml, at)?;
                    let (length, taken) = package_length(aml, at + 4)?;
                    if seg == wanted {
                        let address = u8::try_from(region.1 + bit / 8).ok()?;
                        return (length == 1).then_some((address, (bit % 8) as u8));
                    }
                    bit += length; at += 4 + taken;
                }
            }
        }
        None
    })
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
    /// The adapter's bit in the controller's space (`ac_bit`), where the tables name it.
    pub ac: Option<bool>,
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
            // A gauge sets DISCHARGING at rest too (TI's bq20z45: full on the mains), so the battery decides last.
            on_ac: self.charger.map(|c| c & AC_PRESENT != 0).or(self.ac).unwrap_or(!discharging),
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

/// Reads the battery whole, a transaction a value; `with_charger`: the charger answered at start; `ac`: the adapter's
/// byte and bit in the controller's space (`ac_bit`).
pub fn read<P: Ports>(bus: &mut Smbus<P>, with_charger: bool, ac: Option<(u8, u8)>) -> Result<Reading, SmbusError> {
    let mut word = |command| bus.read_word(BATTERY, command);
    let mut reading = Reading {
        status: word(BATTERY_STATUS)?, percent: word(RELATIVE_CHARGE)?, to_empty: word(TO_EMPTY)?, to_full: word(TO_FULL)?,
        millivolts: word(VOLTAGE)?, current: word(CURRENT)?, remaining: word(REMAINING)?, full: word(FULL_CHARGE)?,
        design: word(DESIGN_CAPACITY)?, mode: word(MODE)?, cycles: word(CYCLES).unwrap_or(0), charger: None, ac: None,
    };
    if with_charger { reading.charger = charger(bus); }
    if let Some((address, bit)) = ac { reading.ac = bus.ec.read(address).map(|byte| byte & (1 << bit) != 0); }
    Ok(reading)
}

/// Minutes as "2:41", or "-" for 65535 (not discharging, or not charging).
pub fn write_minutes(out: &mut impl core::fmt::Write, value: u16) -> core::fmt::Result {
    if value == 0xFFFF { out.write_str("-") } else { write!(out, "{}:{:02}", value / 60, value % 60) }
}
