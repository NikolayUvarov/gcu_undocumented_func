// What an nvme instance sends its controller (213-DRV-0060): the first instance (init's argument empty or 0) every
// command the driver uses; any other instance, or an argument that names none, only Identify, the I/O queues' bring-up
// (their number, then their creation) and Read, so its disk is never written while it is tested. And what a flush finds
// to empty (307-DRV-0055). Built on the host for tests (tests/nvme_host.rs).
use core::fmt;

// Admin commands (NVMe 1.4, figure 139) and I/O commands (figure 346) the driver sends.
pub const OP_CREATE_SQ: u8 = 0x01; pub const OP_CREATE_CQ: u8 = 0x05; pub const OP_IDENTIFY: u8 = 0x06; pub const OP_SET_FEATURES: u8 = 0x09;
pub const OP_GET_FEATURES: u8 = 0x0A;
// Set Features' command dword 10 for the Number of Queues (feature 07h), not saved (SV, bit 31, clear).
pub const QUEUES_FEATURE: u32 = 0x07;
// Get Features' command dword 10 for the Volatile Write Cache (feature 06h), its current value (SEL 0).
pub const VWC_FEATURE: u32 = 0x06;
pub const OP_FLUSH: u8 = 0x00; pub const OP_WRITE: u8 = 0x01; pub const OP_READ: u8 = 0x02;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue { Admin, Io }

// The instance init's argument names: none (init starts the first without one) or a decimal number; None for other text.
pub fn instance(args: &str) -> Option<u32> {
    let word = args.trim();
    if word.is_empty() { return Some(0); }
    if !word.bytes().all(|b| b.is_ascii_digit()) { return None; }
    word.parse().ok()
}

// Whether the instance writes its disk: the first alone; another made writable is a task of its own (213-DRV-0105).
pub fn writes(instance: Option<u32>) -> bool { instance == Some(0) }

// Whether `opcode` with command dword 10 `cdw10` on `queue` reaches the controller: an allow-list for an instance that
// does not write; of Set Features only the number of queues, unsaved, which the queues' bring-up needs.
pub fn allowed(writes: bool, queue: Queue, opcode: u8, cdw10: u32) -> bool {
    writes || match queue {
        Queue::Admin => matches!(opcode, OP_IDENTIFY | OP_CREATE_CQ | OP_CREATE_SQ) || (opcode == OP_SET_FEATURES && cdw10 == QUEUES_FEATURE),
        Queue::Io => opcode == OP_READ,
    }
}

// The command's name for the log.
pub fn name(queue: Queue, opcode: u8) -> &'static str {
    match (queue, opcode) {
        (Queue::Admin, 0x00) => "DELETE I/O SUBMISSION QUEUE", (Queue::Admin, 0x01) => "CREATE I/O SUBMISSION QUEUE",
        (Queue::Admin, 0x02) => "GET LOG PAGE", (Queue::Admin, 0x04) => "DELETE I/O COMPLETION QUEUE",
        (Queue::Admin, 0x05) => "CREATE I/O COMPLETION QUEUE", (Queue::Admin, 0x06) => "IDENTIFY", (Queue::Admin, 0x08) => "ABORT",
        (Queue::Admin, 0x09) => "SET FEATURES", (Queue::Admin, 0x0A) => "GET FEATURES", (Queue::Admin, 0x0C) => "ASYNCHRONOUS EVENT REQUEST",
        (Queue::Admin, 0x0D) => "NAMESPACE MANAGEMENT", (Queue::Admin, 0x10) => "FIRMWARE COMMIT", (Queue::Admin, 0x11) => "FIRMWARE IMAGE DOWNLOAD",
        (Queue::Admin, 0x14) => "DEVICE SELF-TEST", (Queue::Admin, 0x15) => "NAMESPACE ATTACHMENT", (Queue::Admin, 0x80) => "FORMAT NVM",
        (Queue::Admin, 0x81) => "SECURITY SEND", (Queue::Admin, 0x84) => "SANITIZE",
        (Queue::Io, 0x00) => "FLUSH", (Queue::Io, 0x01) => "WRITE", (Queue::Io, 0x02) => "READ", (Queue::Io, 0x04) => "WRITE UNCORRECTABLE",
        (Queue::Io, 0x05) => "COMPARE", (Queue::Io, 0x08) => "WRITE ZEROES", (Queue::Io, 0x09) => "DATASET MANAGEMENT",
        (Queue::Io, 0x0D) => "RESERVATION REGISTER", (Queue::Io, 0x11) => "RESERVATION ACQUIRE", (Queue::Io, 0x15) => "RESERVATION RELEASE",
        _ => "ANOTHER COMMAND",
    }
}

// The instance's tag in the log: [NVME] for the first, as before; [NVME#n] for another; [NVME#?] for none named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag(pub Option<u32>);

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 { Some(0) => f.write_str("[NVME]"), Some(n) => write!(f, "[NVME#{}]", n), None => f.write_str("[NVME#?]") }
    }
}

// Refusals said one line each up to SAID, then every EVERY-th with the count, so a client that asks in a loop cannot fill the log.
pub const SAID: u64 = 64;
pub const EVERY: u64 = 1024;

pub struct Refusals { count: u64 }

impl Refusals {
    pub const fn new() -> Self { Refusals { count: 0 } }
    // One more refusal: Some(its number) when it is said.
    pub fn one(&mut self) -> Option<u64> {
        self.count += 1;
        (self.count <= SAID || self.count % EVERY == 0).then_some(self.count)
    }
}

// What the log adds to refusal `n`: at SAID, that the later ones are counted.
pub fn after(n: u64) -> &'static str { if n == SAID { "; LATER REFUSALS ARE COUNTED, EVERY 1024TH SAID" } else { "" } }

// An Identify string (space-padded ASCII) as text: printable bytes only, trailing spaces and NULs cut.
pub fn ascii<'a>(bytes: &[u8], out: &'a mut [u8]) -> &'a str {
    let mut n = 0;
    for (slot, &b) in out.iter_mut().zip(bytes) { *slot = if (0x20..0x7F).contains(&b) { b } else { b'?' }; n += 1; }
    while n > 0 && (out[n - 1] == b' ' || bytes[n - 1] == 0) { n -= 1; }
    core::str::from_utf8(&out[..n]).unwrap_or("")
}

// The controller's volatile write cache (NVMe 1.4, 5.15.2.2 and 5.21.1.6): absent, present but off, or on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cache { Absent, Off, On }

// From Identify Controller's VWC byte (bit 0: present) and Get Features' answer for the Volatile Write Cache (dword 0,
// bit 0: WCE), None when it was not asked or not answered: then a present cache is taken as on, and a flush is sent.
pub fn cache(vwc: u8, wce: Option<u32>) -> Cache {
    if vwc & 1 == 0 { Cache::Absent } else if wce.is_some_and(|dword| dword & 1 == 0) { Cache::Off } else { Cache::On }
}
