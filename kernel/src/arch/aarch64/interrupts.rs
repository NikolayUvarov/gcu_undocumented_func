// GICv3 (distributor and the CPU's redistributor at their `virt` addresses, the CPU interface through system
// registers) and the EL1 virtual timer as the 100 Hz tick (issue 201). Device lines are SPIs; line n is SPI 32 + n.
// MSI lines (MSI_FIRST + n) are LPIs 8192 + n, raised by the ITS from the writes of the devices (issue 202).
use super::context::{Event, MSI_FIRST};
use core::alloc::Layout;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const GICD: usize = 0x0800_0000;
const GICR: usize = 0x080A_0000; // CPU 0's redistributor; its SGI/PPI page follows at +64 KiB
const TIMER_PPI: u32 = 27; // EL1 virtual timer
const TICK_MS: u64 = 10;
static TICKS: AtomicU64 = AtomicU64::new(0);
static TIMER_STEP: AtomicU64 = AtomicU64::new(0);
// SGIs (software interrupts between CPUs, issue 203): stop, tick, wake.
pub const SGI_STOP: u32 = 1; pub const SGI_TICK: u32 = 2; pub const SGI_WAKE: u32 = 3;

unsafe fn write32(address: usize, value: u32) { core::ptr::write_volatile(address as *mut u32, value) }
unsafe fn read32(address: usize) -> u32 { core::ptr::read_volatile(address as *const u32) }
unsafe fn write64(address: usize, value: u64) { core::ptr::write_volatile(address as *mut u64, value) }
unsafe fn read64(address: usize) -> u64 { core::ptr::read_volatile(address as *const u64) }

pub fn advance() { TICKS.fetch_add(1, Ordering::Relaxed); }
pub fn milliseconds() -> u64 { TICKS.load(Ordering::Relaxed).wrapping_mul(TICK_MS) }

pub fn without<T>(f: impl FnOnce() -> T) -> T {
    unsafe {
        let daif: u64;
        asm!("mrs {}, daif", "msr daifset, #2", out(reg) daif, options(nostack));
        let result = f();
        if daif & (1 << 7) == 0 { asm!("msr daifclr, #2", options(nostack)); }
        result
    }
}

/// The distributor, this CPU's redistributor and CPU interface, and the tick.
pub unsafe fn init() {
    write32(GICD, 1 << 4 | 1 << 1); // ARE_NS, Group 1 non-secure
    // Device lines (SPIs): group 1, to CPU 0, level-triggered as reset; enabled by their drivers.
    let lines = ((read32(GICD + 4) & 0x1F) as usize + 1) * 32;
    for word in 1..lines / 32 { write32(GICD + 0x80 + 4 * word, u32::MAX); }
    for spi in 32..lines { core::ptr::write_volatile((GICD + 0x6000 + 8 * spi) as *mut u64, 0); }
    load();
    let frequency: u64; asm!("mrs {}, cntfrq_el0", out(reg) frequency);
    TIMER_STEP.store(frequency * TICK_MS / 1000, Ordering::Release);
    rearm();
    asm!("msr cntv_ctl_el0, {}", in(reg) 1u64); // enabled, not masked
}

// This CPU's redistributor and CPU interface.
pub unsafe fn load() {
    let waker = GICR + 0x14;
    write32(waker, read32(waker) & !(1 << 1)); // ProcessorSleep off
    while read32(waker) & (1 << 2) != 0 { core::hint::spin_loop(); } // ChildrenAsleep
    let sgi = GICR + 0x1_0000;
    write32(sgi + 0x80, u32::MAX); // IGROUPR0: SGIs and PPIs in group 1
    write32(sgi + 0x100, 1 << TIMER_PPI | 1 << SGI_STOP | 1 << SGI_TICK | 1 << SGI_WAKE); // ISENABLER0
    asm!("msr icc_sre_el1, {}", "isb", in(reg) 7u64);
    asm!("msr icc_pmr_el1, {}", in(reg) 0xFFu64);
    asm!("msr icc_igrpen1_el1, {}", "isb", in(reg) 1u64);
}

unsafe fn rearm() { asm!("msr cntv_tval_el0, {}", in(reg) TIMER_STEP.load(Ordering::Relaxed)); }

// MSI lines are edge-triggered LPIs: like MSI on x86 they are not masked at the controller.
pub fn irq_masked(line: u8) -> bool {
    if line as usize >= MSI_FIRST { return false; }
    let spi = 32 + line as usize;
    unsafe { read32(GICD + 0x100 + spi / 32 * 4) & 1 << (spi % 32) == 0 }
}
pub unsafe fn set_irq_masked(line: u8, masked: bool) {
    if line as usize >= MSI_FIRST { return; }
    let spi = 32 + line as usize;
    write32(GICD + if masked { 0x180 } else { 0x100 } + spi / 32 * 4, 1 << (spi % 32));
}

// Takes the pending interrupt from the CPU interface and ends it; a device line stays disabled until its driver
// acknowledges it.
pub unsafe fn acknowledge() -> Event {
    let intid: u64;
    asm!("mrs {}, icc_iar1_el1", out(reg) intid);
    let intid = intid as u32 & 0xFF_FFFF;
    if (1020..1024).contains(&intid) { return Event::Wake; } // spurious
    let event = match intid {
        TIMER_PPI => { rearm(); advance(); super::cpu::tick_others(); Event::Tick }
        SGI_STOP => Event::Stop,
        SGI_TICK => Event::Tick,
        SGI_WAKE => Event::Wake,
        LPI_FIRST.. => Event::Irq(MSI_FIRST + (intid - LPI_FIRST) as usize),
        32..1020 => { set_irq_masked((intid - 32) as u8, true); Event::Irq((intid - 32) as usize) }
        _ => Event::Wake,
    };
    asm!("msr icc_eoir1_el1, {}", "isb", in(reg) intid as u64);
    event
}

// The ITS (interrupt translation service) of `virt`: device writes to GITS_TRANSLATER carry an event ID; the ITS maps
// (device ID, event) to an LPI for a collection, here collection 0 on CPU 0. Its tables, the command queue and the
// redistributor's LPI configuration and pending tables are taken from the frame pool when the first MSI is routed.
const GITS: usize = 0x0808_0000;
const TRANSLATER: u64 = GITS as u64 + 0x1_0040;
const LPI_FIRST: u32 = 8192;
const LPI_ID_BITS: u64 = 14; // INTIDs below 16384
const LPIS: usize = 16; // MSI lines
const EVENT_BITS: u64 = 4; // events 0..15 per device: the MSI line index
const DEVICES: usize = 16; // devices with an interrupt translation table
const QUEUE_BYTES: usize = 4096;
const CACHED: u64 = 0b111 << 59 | 1 << 10; // inner write-back, inner shareable (table registers)

struct Its { queue: usize, write: usize, devices: [Option<u32>; DEVICES] }
static mut STATE: Option<Its> = None;
static LOCK: AtomicBool = AtomicBool::new(false);

unsafe fn zeroed(bytes: usize, align: usize) -> Option<u64> {
    crate::frames::allocate(Layout::from_size_align(bytes, align).ok()?).map(|p| p.as_ptr() as u64)
}

impl Its {
    unsafe fn start() -> Option<Self> {
        // Redistributor: LPI configuration (priority 0xA0, enabled, for the MSI lines) and pending tables, then LPIs on.
        let properties = zeroed((1usize << LPI_ID_BITS) - LPI_FIRST as usize, 4096)?;
        for lpi in 0..LPIS { core::ptr::write_volatile((properties as usize + lpi) as *mut u8, 0xA0 | 1); }
        let pending = zeroed((1usize << LPI_ID_BITS) / 8, 0x1_0000)?;
        write64(GICR + 0x70, properties | 0b111 << 7 | 1 << 10 | (LPI_ID_BITS - 1));
        write64(GICR + 0x78, pending | 0b111 << 7 | 1 << 10);
        write32(GICR, read32(GICR) | 1); // EnableLPIs
        // ITS tables: device and collection tables, 64 KiB each, flat, 4 KiB pages.
        for n in 0..8 {
            let register = GITS + 0x100 + 8 * n;
            let kind = read64(register) >> 56 & 7;
            if kind != 1 && kind != 4 { continue; }
            let table = zeroed(0x1_0000, 0x1_0000)?;
            let entry = read64(register) >> 48 & 0x1F;
            write64(register, 1 << 63 | CACHED | kind << 56 | entry << 48 | table | 15);
        }
        let queue = zeroed(QUEUE_BYTES, 0x1_0000)? as usize;
        write64(GITS + 0x80, 1 << 63 | CACHED | queue as u64); // GITS_CBASER: one page
        write64(GITS + 0x88, 0); // GITS_CWRITER
        write32(GITS, read32(GITS) | 1); // enabled
        let mut its = Self { queue, write: 0, devices: [None; DEVICES] };
        // Collection 0 targets this CPU: its redistributor's address or its number, as GITS_TYPER.PTA says.
        let target = if read64(GITS + 8) & 1 << 19 != 0 { GICR as u64 } else { 0 };
        its.command([0x09, 0, 1 << 63 | target << 16, 0]); // MAPC
        its.command([0x05, 0, target << 16, 0]); // SYNC
        Some(its)
    }

    // Queues one command and waits until the ITS has read it.
    unsafe fn command(&mut self, words: [u64; 4]) -> bool {
        for (i, word) in words.iter().enumerate() { core::ptr::write_volatile((self.queue + self.write + 8 * i) as *mut u64, *word); }
        asm!("dsb ish");
        self.write = (self.write + 32) % QUEUE_BYTES;
        write64(GITS + 0x88, self.write as u64);
        for _ in 0..1_000_000 { if read64(GITS + 0x90) as usize & (QUEUE_BYTES - 1) == self.write { return true; } core::hint::spin_loop(); }
        false
    }

    // Maps event `index` of device `device` to LPI 8192 + index in collection 0.
    unsafe fn route(&mut self, device: u32, index: usize) -> Option<()> {
        if index >= LPIS { return None; }
        if !self.devices.contains(&Some(device)) {
            let slot = self.devices.iter().position(Option::is_none)?;
            let entry = (read64(GITS + 8) >> 4 & 0xF) + 1; // GITS_TYPER.ITT_entry_size
            let table = zeroed((entry << EVENT_BITS).next_multiple_of(256) as usize, 256)?;
            if !self.command([0x08 | (device as u64) << 32, EVENT_BITS - 1, 1 << 63 | table, 0]) { return None; } // MAPD
            self.devices[slot] = Some(device);
        }
        let mapped = self.command([0x0A | (device as u64) << 32, index as u64 | ((LPI_FIRST as usize + index) as u64) << 32, 0, 0]) // MAPTI
            && self.command([0x0D, 0, 0, 0]) // INVALL: collection 0 rereads the configuration
            && self.command([0x05, 0, 0, 0]); // SYNC
        mapped.then_some(())
    }
}

/// The MSI message of line MSI_FIRST + `index` for the PCI function with requester ID `device`.
pub unsafe fn its_route(device: u32, index: usize) -> Option<(u64, u32)> {
    without(|| {
        while LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); }
        let state = &mut *core::ptr::addr_of_mut!(STATE);
        if state.is_none() { *state = Its::start(); }
        let routed = state.as_mut().and_then(|its| its.route(device, index));
        LOCK.store(false, Ordering::Release);
        routed.map(|()| (TRANSLATER, index as u32))
    })
}
