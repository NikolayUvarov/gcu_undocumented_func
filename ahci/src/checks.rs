// AHCI's parts a host test drives (000-DRV-0064): a wait against the clock to a deadline, the BIOS/OS handoff's steps,
// and the room a request needs, checked here rather than by the caller.

/// How long a register is polled without sleeping.
pub const SPIN_NS: u64 = 2_000_000;
/// A port's command list or FIS receive stopping (the specification's 500 ms), and the device ready after the start.
pub const STOP_NS: u64 = 500_000_000;
pub const READY_NS: u64 = 10_000_000_000;
/// A command's deadline, and a flush's, which may write the drive's whole cache.
pub const COMMAND_NS: u64 = 10_000_000_000;
pub const FLUSH_NS: u64 = 30_000_000_000;
/// The handoff (AHCI 1.3.1, 10.6.3): the firmware lets go within 25 ms, or within 2 s while it says it is busy.
pub const HANDOFF_NS: u64 = 25_000_000;
pub const BUSY_NS: u64 = 2_000_000_000;
/// The most sectors one command moves: the FIS's 16-bit count, 0 not taken.
pub const MAX_SECTORS: usize = 0xFFFF;
/// BOHC's bits: BIOS owned, OS ownership asked, BIOS busy.
pub const BOS: u32 = 1; pub const OOS: u32 = 2; pub const BB: u32 = 1 << 4;

/// The monotonic clock in ns and the shortest sleep.
pub trait Clock { fn now_ns(&self) -> u64; fn sleep(&self); }

/// Whether `done` became true within `deadline_ns`: polled without pause for SPIN_NS, then between sleeps.
pub fn wait<C: Clock>(clock: &C, deadline_ns: u64, mut done: impl FnMut() -> bool) -> bool {
    let start = clock.now_ns();
    loop {
        if done() { return true; }
        let elapsed = clock.now_ns().saturating_sub(start);
        if elapsed >= deadline_ns { return done(); }
        if elapsed >= SPIN_NS { clock.sleep(); } else { core::hint::spin_loop(); }
    }
}

/// The bytes of a request of `count` sectors, if one command moves them and both the caller's buffer and the DMA
/// region's data area hold them; None: refused.
pub fn data_bytes(count: usize, buffer: usize, room: usize) -> Option<usize> {
    let bytes = count.checked_mul(512)?;
    (count != 0 && count <= MAX_SECTORS && bytes <= buffer && bytes <= room).then_some(bytes)
}

/// BOHC, read and written, beside a clock.
pub trait Bohc: Clock { fn bohc(&self) -> u32; fn set_bohc(&self, value: u32); }

/// How the handoff ended: taken, the firmware silent but not busy (taken, as the specification allows), or kept by a
/// firmware still busy after BUSY_NS; with the time it took in ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handoff { Taken(u64), Silent(u64), Kept(u64) }

/// OS ownership asked, then BOS awaited for HANDOFF_NS, and while BB is set for BUSY_NS more.
pub fn handoff<H: Bohc>(hba: &H) -> Handoff {
    let start = hba.now_ns();
    hba.set_bohc(hba.bohc() | OOS);
    let ms = |hba: &H| hba.now_ns().saturating_sub(start) / 1_000_000;
    if wait(hba, HANDOFF_NS, || hba.bohc() & (BOS | BB) == 0) { return Handoff::Taken(ms(hba)); }
    if hba.bohc() & BB == 0 { return if hba.bohc() & BOS == 0 { Handoff::Taken(ms(hba)) } else { Handoff::Silent(ms(hba)) }; }
    if wait(hba, BUSY_NS, || hba.bohc() & (BOS | BB) == 0) { Handoff::Taken(ms(hba)) } else { Handoff::Kept(ms(hba)) }
}
