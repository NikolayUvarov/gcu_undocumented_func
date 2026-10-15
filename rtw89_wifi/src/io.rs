//! Register access and bounded polling shared by the native adapter and host fixtures.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultKind { Access, Unavailable, Timeout, Clock, UnsupportedCut, PowerState, Autoload, Efuse, Mac, Rfe, Xtal }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fault { pub kind: FaultKind, pub register: usize, pub value: u32 }

impl Fault {
    pub const fn new(kind: FaultKind, register: usize, value: u32) -> Self { Self { kind, register, value } }
}

pub type Result<T> = core::result::Result<T, Fault>;

pub trait RegisterIo {
    fn read8(&mut self, at: usize) -> Result<u8>;
    fn read16(&mut self, at: usize) -> Result<u16>;
    fn read32(&mut self, at: usize) -> Result<u32>;
    fn write8(&mut self, at: usize, value: u8) -> Result<()>;
    fn write16(&mut self, at: usize, value: u16) -> Result<()>;
    fn write32(&mut self, at: usize, value: u32) -> Result<()>;
    fn now_us(&mut self) -> u64;
    fn delay_us(&mut self, us: u32) -> Result<()>;
}

pub fn modify8(io: &mut impl RegisterIo, at: usize, clear: u8, set: u8) -> Result<()> {
    let value = io.read8(at)?;
    io.write8(at, (value & !clear) | set)
}

pub fn modify16(io: &mut impl RegisterIo, at: usize, clear: u16, set: u16) -> Result<()> {
    let value = io.read16(at)?;
    io.write16(at, (value & !clear) | set)
}

pub fn read32(io: &mut impl RegisterIo, at: usize) -> Result<u32> {
    let value = io.read32(at)?;
    if value == u32::MAX { return Err(Fault::new(FaultKind::Unavailable, at, value)); }
    Ok(value)
}

pub fn modify32(io: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> Result<()> {
    let value = read32(io, at)?;
    io.write32(at, (value & !clear) | set)
}

pub fn poll32(io: &mut impl RegisterIo, at: usize, mask: u32, expected: u32, interval_us: u32, timeout_us: u32) -> Result<u32> {
    let start = io.now_us();
    let interval = interval_us.max(1);
    let mut value = 0;
    for _ in 0..=u64::from(timeout_us) / u64::from(interval) + 1 {
        let elapsed = io.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock, at, value))?;
        if elapsed >= timeout_us as u64 { break; }
        value = read32(io, at)?;
        let elapsed = io.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock, at, value))?;
        if elapsed >= timeout_us as u64 { break; }
        if value & mask == expected { return Ok(value); }
        io.delay_us(interval.min((timeout_us as u64 - elapsed) as u32))?;
    }
    Err(Fault::new(FaultKind::Timeout, at, value))
}

/// Reject a different function, active DMA, an I/O BAR or an incomplete grant before mapping the device.
pub fn validate_grant(id: u32, command: u16, bar_low: u32, bar_high: u32, len: usize) -> Result<()> {
    if id != 0xb852_10ec || command & 7 != 2 { return Err(Fault::new(FaultKind::Access, 0, id)); }
    let kind = bar_low & 7;
    let base = match kind {
        0 => u64::from(bar_low & !15),
        4 => (u64::from(bar_high) << 32) | u64::from(bar_low & !15),
        _ => return Err(Fault::new(FaultKind::Access, 0x18, bar_low)),
    };
    if base == 0 || len < 0xc004 || base.checked_add(len as u64).is_none() {
        return Err(Fault::new(FaultKind::Access, 0x18, bar_low));
    }
    Ok(())
}

pub fn check_offset(at: usize, width: usize, len: usize) -> Result<()> {
    if !matches!(width, 1 | 2 | 4) || at % width != 0 || at.checked_add(width).is_none_or(|end| end > len) {
        return Err(Fault::new(FaultKind::Access, at, width as u32));
    }
    Ok(())
}

/// One stage's total budget; cleanup gets a separate budget even after an earlier timeout.
pub struct TimedIo<'a, I> { io: &'a mut I, start: u64, timeout_us: u64 }

impl<'a, I: RegisterIo> TimedIo<'a, I> {
    pub fn new(io: &'a mut I, timeout_us: u64) -> Self {
        let start = io.now_us();
        Self { io, start, timeout_us }
    }
    fn remaining(&mut self, at: usize) -> Result<u64> {
        let elapsed = self.io.now_us().checked_sub(self.start).ok_or(Fault::new(FaultKind::Clock, at, 0))?;
        self.timeout_us.checked_sub(elapsed).filter(|left| *left != 0).ok_or(Fault::new(FaultKind::Timeout, at, 0))
    }
}

impl<I: RegisterIo> RegisterIo for TimedIo<'_, I> {
    fn read8(&mut self, at: usize) -> Result<u8> { self.remaining(at)?; self.io.read8(at) }
    fn read16(&mut self, at: usize) -> Result<u16> { self.remaining(at)?; self.io.read16(at) }
    fn read32(&mut self, at: usize) -> Result<u32> { self.remaining(at)?; self.io.read32(at) }
    fn write8(&mut self, at: usize, value: u8) -> Result<()> { self.remaining(at)?; self.io.write8(at, value) }
    fn write16(&mut self, at: usize, value: u16) -> Result<()> { self.remaining(at)?; self.io.write16(at, value) }
    fn write32(&mut self, at: usize, value: u32) -> Result<()> { self.remaining(at)?; self.io.write32(at, value) }
    fn now_us(&mut self) -> u64 { self.io.now_us() }
    fn delay_us(&mut self, us: u32) -> Result<()> {
        if self.remaining(0)? < us as u64 { return Err(Fault::new(FaultKind::Timeout, 0, 0)); }
        self.io.delay_us(us)?;
        self.remaining(0).map(|_| ())
    }
}
