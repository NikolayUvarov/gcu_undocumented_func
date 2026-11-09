// The primary channel's commands and their waits (000-DRV-0064), over a `Bus` so that a host test drives them with a
// stand-in: every wait ends at a deadline on the monotonic clock, and each request is checked here, not by its caller.

pub const DATA: u16 = 0x1F0; pub const COUNT: u16 = 0x1F2; pub const LBA0: u16 = 0x1F3; pub const LBA1: u16 = 0x1F4;
pub const LBA2: u16 = 0x1F5; pub const DRIVE: u16 = 0x1F6; pub const COMMAND: u16 = 0x1F7; pub const CONTROL: u16 = 0x3F6;
pub const BSY: u8 = 0x80; pub const DRQ: u8 = 0x08; pub const ERR: u8 = 0x01;

/// The most sectors one command moves: its count register's 8 bits, 0 meaning 256.
pub const MAX_SECTORS: usize = 256;
/// The end of LBA28's addresses.
pub const LBA28: u64 = 1 << 28;
/// How long the channel is polled without sleeping.
pub const SPIN_NS: u64 = 2_000_000;
/// The deadlines: a command's start and each of its sectors, IDENTIFY's at probe, and a flush, which may write the
/// drive's whole cache.
pub const COMMAND_NS: u64 = 10_000_000_000;
pub const PROBE_NS: u64 = 5_000_000_000;
pub const FLUSH_NS: u64 = 30_000_000_000;

/// The channel's ports (the command block and the control port) and a clock.
pub trait Bus {
    fn in8(&self, port: u16) -> u8;
    fn out8(&self, port: u16, value: u8);
    fn read_words(&self, port: u16, words: &mut [u16]) -> bool;
    fn write_words(&self, port: u16, words: &[u16]) -> bool;
    /// The monotonic clock in ns.
    fn now_ns(&self) -> u64;
    /// The shortest sleep.
    fn sleep(&self);
}

/// Why a request failed: the drive stayed busy, gave no data, or reported an error; or the request was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault { Busy, NoData, Error, Refused }

/// Whether `done` became true within `deadline_ns`: polled without pause for SPIN_NS, then between sleeps.
pub fn wait<B: Bus>(bus: &B, deadline_ns: u64, mut done: impl FnMut() -> bool) -> bool {
    let start = bus.now_ns();
    loop {
        if done() { return true; }
        let elapsed = bus.now_ns().saturating_sub(start);
        if elapsed >= deadline_ns { return done(); }
        if elapsed >= SPIN_NS { bus.sleep(); } else { core::hint::spin_loop(); }
    }
}

pub struct Ide<B: Bus> { pub bus: B, pub sectors: u64 }

impl<B: Bus> Ide<B> {
    /// The primary master, by IDENTIFY: its LBA28 sectors; None without a drive, or with ATAPI or SATA's signature.
    pub fn probe(bus: B) -> Option<Self> {
        bus.out8(CONTROL, 0x02); // nIEN: no interrupts
        bus.out8(DRIVE, 0xA0);
        if matches!(bus.in8(COMMAND), 0xFF | 0x00) { return None; } // a floating bus, or no drive
        for port in [COUNT, LBA0, LBA1, LBA2] { bus.out8(port, 0); }
        bus.out8(COMMAND, 0xEC);
        if bus.in8(COMMAND) == 0 { return None; }
        let mut disk = Self { bus, sectors: 0 };
        disk.idle(PROBE_NS).ok()?;
        if disk.bus.in8(LBA1) != 0 || disk.bus.in8(LBA2) != 0 { return None; }
        disk.data().ok()?;
        let mut identify = [0u16; 256];
        if !disk.bus.read_words(DATA, &mut identify) { return None; }
        disk.sectors = (identify[60] as u64 | (identify[61] as u64) << 16).min(LBA28);
        (disk.sectors != 0).then_some(disk)
    }

    // 400 ns through the alternate status register.
    fn pause(&self) { for _ in 0..4 { self.bus.in8(CONTROL); } }

    // Not busy within `deadline_ns`: the status then.
    fn idle(&self, deadline_ns: u64) -> Result<u8, Fault> {
        let mut status = 0;
        if wait(&self.bus, deadline_ns, || { status = self.bus.in8(COMMAND); status & BSY == 0 }) { Ok(status) } else { Err(Fault::Busy) }
    }

    // The command ended: not busy, and no error.
    fn finished(&self, deadline_ns: u64) -> Result<(), Fault> {
        self.pause();
        if self.idle(deadline_ns)? & ERR != 0 { Err(Fault::Error) } else { Ok(()) }
    }

    // A sector's data ready (DRQ), or the drive's error.
    fn data(&self) -> Result<(), Fault> {
        self.pause();
        let mut status = 0;
        let ready = wait(&self.bus, COMMAND_NS, || { status = self.bus.in8(COMMAND); status & BSY == 0 && status & (DRQ | ERR) != 0 });
        if !ready { return Err(if status & BSY != 0 { Fault::Busy } else { Fault::NoData }); }
        if status & ERR != 0 { Err(Fault::Error) } else { Ok(()) }
    }

    // A request of `count` sectors at `lba` and a buffer of `bytes`: within one command, LBA28, the drive and the buffer.
    fn check(&self, lba: u64, count: usize, bytes: usize) -> Result<(), Fault> {
        let end = lba.checked_add(count as u64).ok_or(Fault::Refused)?;
        let fits = count.checked_mul(512).is_some_and(|need| need <= bytes);
        if count == 0 || count > MAX_SECTORS || !fits || end > self.sectors.min(LBA28) { return Err(Fault::Refused); }
        Ok(())
    }

    fn command(&self, lba: u64, count: usize, command: u8) -> Result<(), Fault> {
        self.idle(COMMAND_NS)?;
        self.bus.out8(DRIVE, 0xE0 | ((lba >> 24) & 0x0F) as u8);
        self.bus.out8(COUNT, count as u8); // 256 is written as 0
        self.bus.out8(LBA0, lba as u8); self.bus.out8(LBA1, (lba >> 8) as u8); self.bus.out8(LBA2, (lba >> 16) as u8);
        self.bus.out8(COMMAND, command);
        Ok(())
    }

    /// READ SECTORS: each sector a DRQ block.
    pub fn read(&self, lba: u64, count: usize, out: &mut [u8]) -> Result<(), Fault> {
        self.check(lba, count, out.len())?;
        self.command(lba, count, 0x20)?;
        let mut words = [0u16; 256];
        for sector in out.chunks_exact_mut(512).take(count) {
            self.data()?;
            if !self.bus.read_words(DATA, &mut words) { return Err(Fault::NoData); }
            for (i, word) in words.iter().enumerate() { sector[i * 2..i * 2 + 2].copy_from_slice(&word.to_le_bytes()); }
        }
        Ok(())
    }

    /// WRITE SECTORS: each sector a DRQ block; the drive is done when BSY clears after the last one.
    pub fn write(&self, lba: u64, count: usize, data: &[u8]) -> Result<(), Fault> {
        self.check(lba, count, data.len())?;
        self.command(lba, count, 0x30)?;
        let mut words = [0u16; 256];
        for sector in data.chunks_exact(512).take(count) {
            for (i, word) in words.iter_mut().enumerate() { *word = u16::from_le_bytes([sector[i * 2], sector[i * 2 + 1]]); }
            self.data()?;
            if !self.bus.write_words(DATA, &words) { return Err(Fault::NoData); }
        }
        self.finished(COMMAND_NS)
    }

    /// FLUSH CACHE.
    pub fn flush(&self) -> Result<(), Fault> {
        self.idle(COMMAND_NS)?;
        self.bus.out8(DRIVE, 0xE0);
        self.bus.out8(COMMAND, 0xE7);
        self.finished(FLUSH_NS)
    }
}
