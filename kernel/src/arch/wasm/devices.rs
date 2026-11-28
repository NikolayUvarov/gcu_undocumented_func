// The page's devices (215-PRT-0017), modelled here so that the page passes plain events: a PL031 on the page's wall
// clock, and two virtio-input devices over virtio-pci on bus 0, a tablet and a keyboard, as QEMU's virtio-tablet-pci and
// virtio-keyboard-pci, so the drivers of aarch64 `virt` serve them unchanged. Their registers lie above 4 GiB, outside
// the module's memory, so the emulator hands a task's accesses there to `read` and `write`; their queues lie in the
// drivers' DMA regions, read and written here directly.

/// The PL031's page and line.
pub const RTC: u64 = 0x1_0000_0000;
pub const RTC_LINE: u32 = 2;
/// Each input device's BAR 0 (64-bit memory, 16 KiB) and line: the tablet at device 1, the keyboard at device 2.
const BARS: [u64; 2] = [0x1_0001_0000, 0x1_0002_0000];
const BAR_SIZE: u64 = 0x4000;
pub const LINES: [u8; 2] = [3, 4];
// The four virtio structures in BAR 0.
const COMMON: u64 = 0x0000;
const NOTIFY: u64 = 0x1000;
const ISR: u64 = 0x2000;
const DEVICE: u64 = 0x3000;
const QUEUE_MAX: u16 = 64;
const PENDING: usize = 256;
// virtio-input configuration selects, and evdev's event types and codes the page sends.
const ID_NAME: u8 = 0x01;
const ID_DEVIDS: u8 = 0x03;
const EV_BITS: u8 = 0x11;
const ABS_INFO: u8 = 0x12;
const EV_KEY: u8 = 1;
const EV_ABS: u8 = 3;
const BTN_LEFT: u16 = 0x110;

#[derive(Clone, Copy)]
struct Queue { size: u16, enable: u16, desc: u64, driver: u64, device: u64, last_avail: u16 }
const QUEUE: Queue = Queue { size: QUEUE_MAX, enable: 0, desc: 0, driver: 0, device: 0, last_avail: 0 };

struct Input {
    command: u16, bar: u64, sizing: [bool; 2],
    status: u8, feature_select: u32, driver_feature_select: u32, queue_select: u16, queues: [Queue; 2],
    select: u8, subsel: u8, isr: u8,
    pending: [(u16, u16, u32); PENDING], head: usize, count: usize,
}
const fn device(bar: u64) -> Input {
    Input { command: 0, bar, sizing: [false; 2], status: 0, feature_select: 0, driver_feature_select: 0, queue_select: 0,
            queues: [QUEUE; 2], select: 0, subsel: 0, isr: 0, pending: [(0, 0, 0); PENDING], head: 0, count: 0 }
}
static mut INPUTS: [Input; 2] = [device(BARS[0]), device(BARS[1])];

unsafe fn inputs() -> &'static mut [Input; 2] { &mut *core::ptr::addr_of_mut!(INPUTS) }

// Bytes of the module's memory, the drivers' DMA regions; out of range reads as zero and writes nothing.
fn ram(at: u64, len: usize) -> Option<*mut u8> {
    let end = at.checked_add(len as u64)?;
    (at >= 4096 && end <= super::run::memory_bytes() as u64).then_some(at as usize as *mut u8)
}
fn ram16(at: u64) -> u16 { ram(at, 2).map_or(0, |p| unsafe { core::ptr::read_unaligned(p as *const u16) }) }
fn ram32(at: u64) -> u32 { ram(at, 4).map_or(0, |p| unsafe { core::ptr::read_unaligned(p as *const u32) }) }
fn ram64(at: u64) -> u64 { ram(at, 8).map_or(0, |p| unsafe { core::ptr::read_unaligned(p as *const u64) }) }
fn set_ram16(at: u64, value: u16) { if let Some(p) = ram(at, 2) { unsafe { core::ptr::write_unaligned(p as *mut u16, value) } } }
fn set_ram32(at: u64, value: u32) { if let Some(p) = ram(at, 4) { unsafe { core::ptr::write_unaligned(p as *mut u32, value) } } }

/// PCI configuration dword `offset` of bus 0's device `slot`: the inputs at 1 and 2, nothing else.
pub fn config_read(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let Some(n) = which(bus, slot, function) else { return u32::MAX };
    let d = unsafe { &inputs()[n] };
    let cap = |next: u32, len: u32, kind: u32| 0x09 | next << 8 | len << 16 | kind << 24;
    match offset & 0xFC {
        0x00 => 0x1052_1AF4,
        0x04 => d.command as u32 | 0x0010 << 16, // the capabilities list
        0x08 => 0x0980_0001, // an input device, other; revision 1 (modern only)
        0x10 => if d.sizing[0] { (!(BAR_SIZE - 1) as u32) | 0x4 } else { d.bar as u32 & 0xFFFF_FFF0 | 0x4 },
        0x14 => if d.sizing[1] { u32::MAX } else { (d.bar >> 32) as u32 },
        0x2C => 0x1100_1AF4,
        0x34 => 0x40,
        0x3C => LINES[n] as u32 | 1 << 8,
        0x40 => cap(0x50, 16, 1), 0x44 => 0, 0x48 => COMMON as u32, 0x4C => 0x38,
        0x50 => cap(0x64, 20, 2), 0x54 => 0, 0x58 => NOTIFY as u32, 0x5C => 4, 0x60 => 0,
        0x64 => cap(0x74, 16, 3), 0x68 => 0, 0x6C => ISR as u32, 0x70 => 4,
        0x74 => cap(0x00, 16, 4), 0x78 => 0, 0x7C => DEVICE as u32, 0x80 => 0x88,
        _ => 0,
    }
}

pub fn config_write(bus: u8, slot: u8, function: u8, offset: u8, value: u32) {
    let Some(n) = which(bus, slot, function) else { return };
    let d = unsafe { &mut inputs()[n] };
    match offset & 0xFC {
        0x04 => d.command = value as u16,
        // A BAR sized (all ones written) answers its size until it is written again.
        0x10 => { d.sizing[0] = value == u32::MAX; if !d.sizing[0] { d.bar = d.bar & !0xFFFF_FFFF | (value & 0xFFFF_FFF0) as u64; } }
        0x14 => { d.sizing[1] = value == u32::MAX; if !d.sizing[1] { d.bar = d.bar & 0xFFFF_FFFF | (value as u64) << 32; } }
        _ => {}
    }
}

fn which(bus: u8, slot: u8, function: u8) -> Option<usize> { (bus == 0 && function == 0 && (1..=2).contains(&slot)).then(|| slot as usize - 1) }

/// A task's read of a page that is not RAM: the PL031's or an input device's registers.
pub fn read(pa: u64, size: u32) -> Option<u64> {
    if (RTC..RTC + 0x1000).contains(&pa) {
        let id = [0x31u8, 0x10, 0x04, 0x00, 0x0D, 0xF0, 0x05, 0xB1];
        return Some(match pa - RTC {
            0x000 => (unsafe { super::host::wall_s() }) & 0xFFFF_FFFF, // RTCDR: seconds since 1970, UTC
            0x00C => 1, // RTCCR: running
            at @ 0xFE0..=0xFFC => id[(at - 0xFE0) as usize / 4] as u64,
            _ => 0,
        });
    }
    let (n, at) = bar_of(pa)?;
    let d = unsafe { &mut inputs()[n] };
    let mut bytes = [0u8; 0x88];
    let window: &[u8] = match at {
        COMMON..0x0038 => { common_image(d, &mut bytes); &bytes[..0x38] }
        ISR..0x2004 => { let isr = d.isr; d.isr = 0; return Some(isr as u64); }
        DEVICE..0x3088 => { device_image(n, d, &mut bytes); &bytes[..0x88] }
        _ => return Some(0),
    };
    let base = at & 0xFFF;
    let mut value = [0u8; 8];
    for i in 0..size.min(8) as usize { value[i] = *window.get(base as usize + i).unwrap_or(&0); }
    Some(u64::from_le_bytes(value))
}

/// A task's write to a page that is not RAM; false where nothing answers, which faults the task.
pub fn write(pa: u64, size: u32, value: u64) -> bool {
    if (RTC..RTC + 0x1000).contains(&pa) { return true; } // the page's clock is not set from here
    let Some((n, at)) = bar_of(pa) else { return false };
    let d = unsafe { &mut inputs()[n] };
    match at {
        COMMON..0x0038 => common_write(d, at as usize, size, value),
        NOTIFY..0x1004 => { if d.queues[0].enable != 0 { deliver(n); } }
        DEVICE..0x3002 => { for i in 0..size as u64 { match at - DEVICE + i { 0 => d.select = (value >> (8 * i)) as u8, 1 => d.subsel = (value >> (8 * i)) as u8, _ => {} } } }
        _ => {}
    }
    true
}

fn bar_of(pa: u64) -> Option<(usize, u64)> {
    let all = unsafe { inputs() };
    (0..2).find(|&n| (all[n].bar..all[n].bar + BAR_SIZE).contains(&pa)).map(|n| (n, pa - all[n].bar))
}

fn common_image(d: &Input, out: &mut [u8; 0x88]) {
    let q = d.queues[d.queue_select as usize % 2];
    let mut put = |at: usize, bytes: &[u8]| out[at..at + bytes.len()].copy_from_slice(bytes);
    put(0x00, &d.feature_select.to_le_bytes());
    put(0x04, &(if d.feature_select == 1 { 1u32 } else { 0 }).to_le_bytes()); // VIRTIO_F_VERSION_1 alone
    put(0x08, &d.driver_feature_select.to_le_bytes());
    put(0x10, &0xFFFFu16.to_le_bytes()); // no MSI-X
    put(0x12, &2u16.to_le_bytes());
    put(0x14, &[d.status, 0]);
    put(0x16, &d.queue_select.to_le_bytes());
    put(0x18, &q.size.to_le_bytes());
    put(0x1A, &0xFFFFu16.to_le_bytes());
    put(0x1C, &q.enable.to_le_bytes());
    put(0x1E, &d.queue_select.to_le_bytes());
    put(0x20, &q.desc.to_le_bytes());
    put(0x28, &q.driver.to_le_bytes());
    put(0x30, &q.device.to_le_bytes());
}

fn common_write(d: &mut Input, at: usize, size: u32, value: u64) {
    let q = d.queue_select as usize % 2;
    let half = |old: u64, high: bool| if high { old & 0xFFFF_FFFF | value << 32 } else { old & !0xFFFF_FFFF | value & 0xFFFF_FFFF };
    match (at, size) {
        (0x00, _) => d.feature_select = value as u32,
        (0x08, _) => d.driver_feature_select = value as u32,
        // A status of 0 resets the device: its queues forgotten, its events kept for the next driver.
        (0x14, _) => { d.status = value as u8; if d.status == 0 { d.queues = [QUEUE; 2]; d.isr = 0; } }
        (0x16, _) => d.queue_select = value as u16,
        (0x18, _) => d.queues[q].size = (value as u16).clamp(1, QUEUE_MAX),
        (0x1C, _) => d.queues[q].enable = value as u16,
        (0x20 | 0x24, 4) => d.queues[q].desc = half(d.queues[q].desc, at == 0x24),
        (0x28 | 0x2C, 4) => d.queues[q].driver = half(d.queues[q].driver, at == 0x2C),
        (0x30 | 0x34, 4) => d.queues[q].device = half(d.queues[q].device, at == 0x34),
        (0x20, 8) => d.queues[q].desc = value,
        (0x28, 8) => d.queues[q].driver = value,
        (0x30, 8) => d.queues[q].device = value,
        _ => {}
    }
}

// The selected configuration: its size at 2, its bytes from 8.
fn device_image(n: usize, d: &Input, out: &mut [u8; 0x88]) {
    out[0] = d.select;
    out[1] = d.subsel;
    let data = &mut out[8..];
    let tablet = n == 0;
    let size = match d.select {
        ID_NAME => { let name: &[u8] = if tablet { b"MIND page tablet" } else { b"MIND page keyboard" }; data[..name.len()].copy_from_slice(name); name.len() }
        ID_DEVIDS => { data[..8].copy_from_slice(&[0x06, 0, 0x27, 0x06, if tablet { 3 } else { 1 }, 0, 1, 0]); 8 } // virtual bus, QEMU's IDs
        EV_BITS if d.subsel == EV_KEY && tablet => { for code in [BTN_LEFT, BTN_LEFT + 1, BTN_LEFT + 2] { data[code as usize / 8] |= 1 << (code % 8); } (BTN_LEFT as usize + 2) / 8 + 1 }
        EV_BITS if d.subsel == EV_KEY => { for code in 1..=248usize { data[code / 8] |= 1 << (code % 8); } 32 }
        EV_BITS if d.subsel == EV_ABS && tablet => { data[0] = 0b11; 1 } // ABS_X, ABS_Y
        ABS_INFO if tablet && d.subsel <= 1 => { data[4..8].copy_from_slice(&32767u32.to_le_bytes()); 20 } // 0 to 32767
        _ => 0,
    };
    out[2] = size as u8;
}

/// An event from the page for device `n` (0 the tablet, 1 the keyboard): evdev's type, code and value. Kept until
/// the driver has a buffer for it; the oldest is dropped when PENDING wait.
pub fn input(n: usize, kind: u16, code: u16, value: u32) {
    if n > 1 { return; }
    let d = unsafe { &mut inputs()[n] };
    if d.count == PENDING { d.head = (d.head + 1) % PENDING; d.count -= 1; }
    d.pending[(d.head + d.count) % PENDING] = (kind, code, value);
    d.count += 1;
    deliver(n);
}

// Events into the buffers the driver offered on queue 0, each 8 bytes (le16 type, le16 code, le32 value); the line
// rises when any went.
fn deliver(n: usize) {
    let d = unsafe { &mut inputs()[n] };
    let q = &mut d.queues[0];
    if d.status & 4 == 0 || q.enable == 0 || q.driver == 0 { return; }
    let mut sent = false;
    while d.count > 0 && ram16(q.driver + 2) != q.last_avail {
        let id = ram16(q.driver + 4 + 2 * (q.last_avail % q.size) as u64);
        let desc = q.desc + 16 * (id % q.size) as u64;
        let (buffer, len) = (ram64(desc), ram32(desc + 8));
        let (kind, code, value) = d.pending[d.head];
        if len >= 8 { set_ram16(buffer, kind); set_ram16(buffer + 2, code); set_ram32(buffer + 4, value); }
        let used = ram16(q.device + 2);
        set_ram32(q.device + 4 + 8 * (used % q.size) as u64, id as u32);
        set_ram32(q.device + 8 + 8 * (used % q.size) as u64, 8);
        set_ram16(q.device + 2, used.wrapping_add(1));
        q.last_avail = q.last_avail.wrapping_add(1);
        d.head = (d.head + 1) % PENDING;
        d.count -= 1;
        sent = true;
    }
    if sent { d.isr |= 1; super::interrupts::raise(LINES[n] as usize); }
}
