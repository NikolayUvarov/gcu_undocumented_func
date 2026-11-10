// AHCI's parts a host test drives (000-DRV-0064): a wait against the clock to a deadline, the BIOS/OS handoff's steps,
// the room a request needs, checked here rather than by the caller, and a port's signature received before it is read
// (307-DRV-0068).

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
/// A port's registers, from its base, and PxCMD's bits: start, FIS receive enable, FIS receive and command list running.
pub const CLB: usize = 0x00; pub const FB: usize = 0x08; pub const CMD: usize = 0x18; pub const TFD: usize = 0x20; pub const SIG: usize = 0x24;
pub const SSTS: usize = 0x28; pub const SCTL: usize = 0x2C; pub const SERR: usize = 0x30;
pub const CMD_ST: u32 = 1; pub const CMD_FRE: u32 = 1 << 4; pub const CMD_FR: u32 = 1 << 14; pub const CMD_CR: u32 = 1 << 15;
/// SIG before the port's first D2H register FIS (AHCI 3.3.9), and an ATA disk's signature.
pub const NO_SIGNATURE: u32 = u32::MAX; pub const DISK: u32 = 0x0000_0101;
/// The device's first D2H register FIS once FIS receive is on, and again after a COMRESET.
pub const SIGNATURE_NS: u64 = 10_000_000_000;
/// A COMRESET's DET 1 (at least 1 ms, AHCI 10.4.2), and the link coming back after it.
pub const COMRESET_NS: u64 = 1_000_000; pub const LINK_NS: u64 = 1_000_000_000;

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

/// An HBA's port registers, by port, beside a clock.
pub trait Ports: Clock { fn reg(&self, port: usize, offset: usize) -> u32; fn set(&self, port: usize, offset: usize, value: u32); }

/// What a port with a device present (SSTS.DET 3) answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Its signature, and whether it came only after a COMRESET.
    Signature(u32, bool),
    /// None after the COMRESET either: SSTS, TFD and SIG as they were left.
    Silent { ssts: u32, tfd: u32, sig: u32 },
    /// Its command list or FIS receive did not stop, so its areas were not set.
    Stuck,
}

/// Port `p`'s command list, then its FIS receive, stopped (AHCI 10.1.2), each awaited for STOP_NS.
pub fn stop<P: Ports>(hba: &P, p: usize) -> bool {
    hba.set(p, CMD, hba.reg(p, CMD) & !CMD_ST);
    if !wait(hba, STOP_NS, || hba.reg(p, CMD) & CMD_CR == 0) { return false; }
    hba.set(p, CMD, hba.reg(p, CMD) & !CMD_FRE);
    wait(hba, STOP_NS, || hba.reg(p, CMD) & CMD_FR == 0)
}

/// Port `p`'s signature, received before SIG is read (AHCI 1.3.1, 3.3.9 and 10.1.2): the port stopped, `list` and `fis`
/// set as its command list and FIS area, FIS receive on, then SIG, or a TFD neither busy nor asking for data, awaited for
/// SIGNATURE_NS; without a signature, a COMRESET (10.4.2) and one more wait.
pub fn signature<P: Ports>(hba: &P, p: usize, list: u64, fis: u64) -> Answer {
    if !stop(hba, p) { return Answer::Stuck; }
    hba.set(p, CLB, list as u32); hba.set(p, CLB + 4, (list >> 32) as u32);
    hba.set(p, FB, fis as u32); hba.set(p, FB + 4, (fis >> 32) as u32);
    hba.set(p, SERR, u32::MAX);
    hba.set(p, CMD, hba.reg(p, CMD) | CMD_FRE);
    let answered = || hba.reg(p, SIG) != NO_SIGNATURE || hba.reg(p, TFD) & 0x88 == 0;
    wait(hba, SIGNATURE_NS, answered);
    let sig = hba.reg(p, SIG);
    if sig != NO_SIGNATURE { return Answer::Signature(sig, false); }
    // COMRESET with ST clear and FRE kept on, so that the device's FIS after it is received; then SERR cleared.
    let sctl = hba.reg(p, SCTL) & !0xF;
    hba.set(p, SCTL, sctl | 1);
    wait(hba, COMRESET_NS, || false);
    hba.set(p, SCTL, sctl);
    if wait(hba, LINK_NS, || hba.reg(p, SSTS) & 0xF == 3) {
        hba.set(p, SERR, u32::MAX);
        wait(hba, SIGNATURE_NS, answered);
        let sig = hba.reg(p, SIG);
        if sig != NO_SIGNATURE { return Answer::Signature(sig, true); }
    }
    Answer::Silent { ssts: hba.reg(p, SSTS), tfd: hba.reg(p, TFD), sig: hba.reg(p, SIG) }
}

/// The first implemented port with a device present whose signature is an ATA disk's, `heard` told each port's answer;
/// a port given our areas and not taken is stopped again, so that it writes no more into the FIS area the ports share.
pub fn disk_port<P: Ports>(hba: &P, implemented: u32, list: u64, fis: u64, mut heard: impl FnMut(usize, Answer)) -> Option<usize> {
    (0..32).filter(|&p| implemented & 1 << p != 0 && hba.reg(p, SSTS) & 0xF == 3).find(|&p| {
        let answer = signature(hba, p, list, fis);
        heard(p, answer);
        let disk = matches!(answer, Answer::Signature(DISK, _));
        if !disk && answer != Answer::Stuck { stop(hba, p); }
        disk
    })
}
