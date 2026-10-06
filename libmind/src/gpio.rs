//! Pin controllers (issue 207): the registers of the BCM2711's GPIO and of Arm's PL061 behind one model (functions,
//! levels, pulls), the hwdocs tables that name a SoC's alternate functions and a board's header, and who may change a
//! pin. No system calls: tests/gpio_host.rs; the `gpio` service drives the hardware with it.

/// Pin functions as `idl/gpio.wit` numbers them: input, output, then the alternate functions ALT0, ALT1, ...
pub const INPUT: u8 = 0;
pub const OUTPUT: u8 = 1;
pub const ALT0: u8 = 2;

/// A client with this badge may change pins that are not reserved; one with `BADGE_PLATFORM` reserved ones too.
pub const BADGE_CONTROL: u16 = 1;
pub const BADGE_PLATFORM: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pull { None, Up, Down, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal { NoPin, Reserved, Denied, Unsupported }

/// Who may change pin `reserved` or not: Ok, or why not (MC-3.3, MC-8.3: driving pins is separate from reading them).
pub fn may_change(badge: u16, reserved: bool) -> Result<(), Refusal> {
    match badge {
        BADGE_PLATFORM => Ok(()),
        BADGE_CONTROL if !reserved => Ok(()),
        BADGE_CONTROL => Err(Refusal::Reserved),
        _ => Err(Refusal::Denied),
    }
}

/// A controller's register window.
pub trait Registers {
    fn read32(&self, offset: usize) -> u32;
    fn write32(&self, offset: usize, value: u32);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Bcm2711, Pl061 }

impl Kind {
    pub fn pins(self) -> u8 { match self { Self::Bcm2711 => 58, Self::Pl061 => 8 } }
    /// How many functions each pin has: input, output and the alternates.
    pub fn functions(self) -> u8 { match self { Self::Bcm2711 => 8, Self::Pl061 => 2 } }
    pub fn name(self) -> &'static str { match self { Self::Bcm2711 => "bcm2711", Self::Pl061 => "pl061" } }
}

// BCM2711: GPFSELn (3 bits a pin, ten pins a register), GPSETn, GPCLRn, GPLEVn, GPIO_PUP_PDN_CNTRL_REGn (2 bits a pin).
const FSEL: usize = 0x00; const SET: usize = 0x1C; const CLR: usize = 0x28; const LEV: usize = 0x34; const PULL: usize = 0xE4;
// FSEL codes in function order: input, output, ALT0..ALT5 (BCM2711 ARM Peripherals, 5.2).
const FSEL_CODES: [u32; 8] = [0b000, 0b001, 0b100, 0b101, 0b110, 0b111, 0b011, 0b010];
// PL061: DATA through its address mask (bits 9:2 select the pins), DIR (1: output).
const PL061_DATA: usize = 0x000; const PL061_DIR: usize = 0x400;

/// A controller of `kind` at `registers`.
pub struct Controller<R: Registers> { pub kind: Kind, pub registers: R }

impl<R: Registers> Controller<R> {
    fn check(&self, pin: u8) -> Result<usize, Refusal> { if pin < self.kind.pins() { Ok(pin as usize) } else { Err(Refusal::NoPin) } }

    pub fn function(&self, pin: u8) -> Result<u8, Refusal> {
        let pin = self.check(pin)?;
        Ok(match self.kind {
            Kind::Bcm2711 => {
                let code = self.registers.read32(FSEL + pin / 10 * 4) >> (pin % 10 * 3) & 7;
                FSEL_CODES.iter().position(|&c| c == code).unwrap_or(0) as u8
            }
            Kind::Pl061 => (self.registers.read32(PL061_DIR) >> pin & 1) as u8,
        })
    }

    pub fn set_function(&self, pin: u8, function: u8) -> Result<(), Refusal> {
        let at = self.check(pin)?;
        if function >= self.kind.functions() { return Err(Refusal::Unsupported); }
        match self.kind {
            Kind::Bcm2711 => {
                let (register, shift) = (FSEL + at / 10 * 4, at % 10 * 3);
                let value = self.registers.read32(register) & !(7 << shift) | FSEL_CODES[function as usize] << shift;
                self.registers.write32(register, value);
            }
            Kind::Pl061 => {
                let dir = self.registers.read32(PL061_DIR) & !(1 << at) | (function as u32) << at;
                self.registers.write32(PL061_DIR, dir);
            }
        }
        Ok(())
    }

    pub fn level(&self, pin: u8) -> Result<bool, Refusal> {
        let pin = self.check(pin)?;
        Ok(match self.kind {
            Kind::Bcm2711 => self.registers.read32(LEV + pin / 32 * 4) >> (pin % 32) & 1 != 0,
            Kind::Pl061 => self.registers.read32(PL061_DATA + (1 << (pin + 2))) != 0,
        })
    }

    /// Drives an output pin; Unsupported unless the pin is an output.
    pub fn write(&self, pin: u8, high: bool) -> Result<(), Refusal> {
        if self.function(pin)? != OUTPUT { return Err(Refusal::Unsupported); }
        let pin = pin as usize;
        match self.kind {
            Kind::Bcm2711 => self.registers.write32(if high { SET } else { CLR } + pin / 32 * 4, 1 << (pin % 32)),
            Kind::Pl061 => self.registers.write32(PL061_DATA + (1 << (pin + 2)), if high { 0xFF } else { 0 }),
        }
        Ok(())
    }

    pub fn pull(&self, pin: u8) -> Result<Pull, Refusal> {
        let pin = self.check(pin)?;
        Ok(match self.kind {
            Kind::Bcm2711 => match self.registers.read32(PULL + pin / 16 * 4) >> (pin % 16 * 2) & 3 { 0 => Pull::None, 1 => Pull::Up, 2 => Pull::Down, _ => Pull::Unknown },
            Kind::Pl061 => Pull::Unknown, // a PL061 has no pulls; the pad's are the SoC's
        })
    }

    pub fn set_pull(&self, pin: u8, pull: Pull) -> Result<(), Refusal> {
        let at = self.check(pin)?;
        let code = match (self.kind, pull) { (Kind::Bcm2711, Pull::None) => 0, (Kind::Bcm2711, Pull::Up) => 1, (Kind::Bcm2711, Pull::Down) => 2, _ => return Err(Refusal::Unsupported) };
        let (register, shift) = (PULL + at / 16 * 4, at % 16 * 2);
        self.registers.write32(register, self.registers.read32(register) & !(3 << shift) | code << shift);
        Ok(())
    }
}

/// A short name from a hwdocs table (a function, a SoC, a board).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Name { bytes: [u8; 24], len: u8 }

impl Name {
    pub fn new(text: &str) -> Self { let mut n = Self::default(); let len = text.len().min(24); n.bytes[..len].copy_from_slice(&text.as_bytes()[..len]); n.len = len as u8; n }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("?") }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

pub const MAX_PINS: usize = 64;

/// A SoC's pin table (`hwdocs/socs/<soc>.pins`): each pin's pull at reset and the names of ALT0.. (empty: none).
pub struct Pins { pub soc: Name, pub count: u8, pub pulls: [Pull; MAX_PINS], pub alternates: [[Name; 6]; MAX_PINS] }

impl Pins {
    pub fn parse(text: &str) -> Option<Self> {
        let mut table = Self { soc: Name::default(), count: 0, pulls: [Pull::Unknown; MAX_PINS], alternates: [[Name::default(); 6]; MAX_PINS] };
        for line in lines(text) {
            let mut words = line.split_whitespace();
            match words.next()? {
                "soc" => table.soc = Name::new(words.next()?),
                "pins" => table.count = words.next()?.parse::<u8>().ok().filter(|&n| n as usize <= MAX_PINS)?,
                "source" => {}
                number => {
                    let pin: usize = number.parse().ok()?;
                    if pin >= table.count as usize { return None; }
                    table.pulls[pin] = match words.next()? { "up" => Pull::Up, "down" => Pull::Down, "none" => Pull::None, _ => return None };
                    for (slot, word) in table.alternates[pin].iter_mut().zip(words) { *slot = if word == "-" { Name::default() } else { Name::new(word) }; }
                }
            }
        }
        (!table.soc.is_empty() && table.count > 0).then_some(table)
    }
    /// The name of `function` on `pin`: "input", "output", an alternate's name, or None if it has none.
    pub fn function_name(&self, pin: u8, function: u8) -> Option<&str> {
        match function {
            INPUT => Some("input"), OUTPUT => Some("output"),
            f => self.alternates.get(pin as usize)?.get((f - ALT0) as usize).filter(|n| !n.is_empty()).map(Name::as_str),
        }
    }
}

/// A board (`hwdocs/boards/<board>.board`): its SoC, the header position of each pin on it, the reserved pins.
pub struct Board { pub board: Name, pub soc: Name, pub positions: [u8; MAX_PINS], pub reserved: u64 }

impl Board {
    pub fn parse(text: &str) -> Option<Self> {
        let mut board = Self { board: Name::default(), soc: Name::default(), positions: [0; MAX_PINS], reserved: 0 };
        for line in lines(text) {
            let mut words = line.split_whitespace();
            match words.next()? {
                "board" => board.board = Name::new(words.next()?),
                "soc" => board.soc = Name::new(words.next()?),
                "name" | "source" | "header" => {}
                "reserved" => {
                    let range = words.next()?;
                    let (low, high) = range.split_once('-').unwrap_or((range, range));
                    let (low, high): (usize, usize) = (low.parse().ok()?, high.parse().ok()?);
                    if low > high || high >= MAX_PINS { return None; }
                    for pin in low..=high { board.reserved |= 1 << pin; }
                }
                position => {
                    let position: u8 = position.parse().ok()?;
                    let pin: usize = words.next()?.parse().ok()?;
                    *board.positions.get_mut(pin)? = position;
                }
            }
        }
        (!board.board.is_empty() && !board.soc.is_empty()).then_some(board)
    }
    pub fn reserved(&self, pin: u8) -> bool { pin as usize >= MAX_PINS || self.reserved >> pin & 1 != 0 }
}

// The lines that say something: without comments (from '#') and blank lines.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty())
}
