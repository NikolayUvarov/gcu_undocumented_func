//! The system log (`logd`, idl/log.wit). A process that holds a log client in SLOT_LOG — `init` and the boot services
//! — sends every line of its `println!` output there as well (complete lines, at most 200 bytes each), and `write`
//! adds a line with a level. `logd` stamps the source (PID and task name) from the sender; a line cannot name its own
//! source. Reading (`read`, `state`) needs the read badge, which only the shell's client carries; the shell lends it
//! to programs that ask for the log (`REQUEST_LOG`, `dmesg`).
use crate::abi::*;
use crate::idl::{log, wire};
use crate::ipc::Endpoint;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

pub use log::{Entry, State};

pub const DEBUG: u8 = 0;
pub const INFO: u8 = 1;
pub const WARN: u8 = 2;
pub const ERROR: u8 = 3;

/// Bytes of a line (longer lines are cut on a character boundary).
pub const LINE: usize = 200;
const BACKLOG: usize = 2048;

// Whether SLOT_LOG holds an endpoint: not looked at yet, yes, no.
const UNKNOWN: u8 = 0;
const PRESENT: u8 = 1;
const ABSENT: u8 = 2;
static STATUS: AtomicU8 = AtomicU8::new(UNKNOWN);
static ENDPOINT: AtomicUsize = AtomicUsize::new(SLOT_LOG);
static CAPTURE: AtomicBool = AtomicBool::new(true);
static BUSY: AtomicBool = AtomicBool::new(false); // a line is being sent (no output from inside the sending)

// The line being collected, lines kept until there is a log to send them to (init's first lines), the shared buffer.
struct Client { line: [u8; LINE], len: usize, backlog: [u8; BACKLOG], kept: usize, shared: Option<wire::Shared> }
struct Cell(UnsafeCell<Client>);
unsafe impl Sync for Cell {} // processes are single-threaded
static CLIENT: Cell = Cell(UnsafeCell::new(Client { line: [0; LINE], len: 0, backlog: [0; BACKLOG], kept: 0, shared: None }));

fn client() -> &'static mut Client { unsafe { &mut *CLIENT.0.get() } }

fn endpoint() -> Option<Endpoint> {
    let status = match STATUS.load(Ordering::Relaxed) {
        UNKNOWN => { let s = if crate::dev::cap_info(SLOT_LOG).0 == CAP_KIND_ENDPOINT { PRESENT } else { ABSENT }; STATUS.store(s, Ordering::Relaxed); s }
        s => s,
    };
    (status == PRESENT).then(|| Endpoint(ENDPOINT.load(Ordering::Relaxed)))
}

// The longest valid UTF-8 prefix.
fn valid(bytes: &[u8]) -> &str {
    match core::str::from_utf8(bytes) { Ok(s) => s, Err(e) => core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or("") }
}

/// Creates the buffer for writing up front if the process holds a log client (called before `main`): services do not
/// grow while they run.
pub fn prepare() {
    if endpoint().is_some() && client().shared.is_none() { client().shared = wire::Shared::new(4096).ok(); }
}

fn send(endpoint: Endpoint, level: u8, text: &str) -> crate::sys::Result<()> {
    let c = client();
    if c.shared.is_none() { c.shared = Some(wire::Shared::new(4096)?); }
    let shared = c.shared.as_mut().unwrap();
    log::write(endpoint, shared.buffer(), level, text)?.map_err(|_| crate::sys::Error::Rights)
}

/// Adds a line with a level (`DEBUG`..`ERROR`) to the system log; nothing happens without a log client.
pub fn write(level: u8, text: &str) -> crate::sys::Result<()> {
    let Some(endpoint) = endpoint() else { return Err(crate::sys::Error::NotFound) };
    if BUSY.swap(true, Ordering::Acquire) { return Err(crate::sys::Error::Other(ERR_BUSY)); }
    let mut end = text.len().min(LINE);
    while !text.is_char_boundary(end) { end -= 1; }
    let result = send(endpoint, level, &text[..end]);
    BUSY.store(false, Ordering::Release);
    result
}

/// Sends further lines to `endpoint` (a log client held elsewhere than SLOT_LOG: init mints its own once `logd` runs),
/// starting with the lines kept so far.
pub fn use_endpoint(endpoint: Endpoint) {
    ENDPOINT.store(endpoint.0, Ordering::Relaxed);
    STATUS.store(PRESENT, Ordering::Relaxed);
    let c = client();
    let mut kept = [0u8; BACKLOG];
    let count = c.kept;
    kept[..count].copy_from_slice(&c.backlog[..count]);
    c.kept = 0;
    for line in kept[..count].split(|&b| b == b'\n').filter(|l| !l.is_empty()) { let _ = write(INFO, valid(line)); }
}

/// Output stays in the process (the console and `LOGS`): for programs that hold a log client to read it (`dmesg`).
pub fn keep_output_local() { CAPTURE.store(false, Ordering::Relaxed); }

/// Called with every piece of `print!` output: complete lines go to the log, or are kept (the last 2 KiB) until there
/// is one.
pub(crate) fn capture(bytes: &[u8]) {
    if !CAPTURE.load(Ordering::Relaxed) || BUSY.load(Ordering::Relaxed) { return; }
    let c = client();
    for &byte in bytes {
        if byte != b'\n' { if c.len < LINE { c.line[c.len] = byte; c.len += 1; } continue; }
        let len = core::mem::take(&mut c.len);
        if len == 0 { continue; }
        let mut line = [0u8; LINE];
        line[..len].copy_from_slice(&c.line[..len]);
        if endpoint().is_some() { let _ = write(INFO, valid(&line[..len])); continue; }
        // No log yet: keep the line, dropping the oldest ones when the backlog is full.
        let need = len + 1;
        if need > BACKLOG { continue; }
        while c.kept + need > BACKLOG {
            let first = c.backlog[..c.kept].iter().position(|&b| b == b'\n').map_or(c.kept, |i| i + 1);
            c.backlog.copy_within(first..c.kept, 0); c.kept -= first;
        }
        c.backlog[c.kept..c.kept + len].copy_from_slice(&line[..len]);
        c.backlog[c.kept + len] = b'\n';
        c.kept += need;
    }
}

/// Records from sequence number `from` on (from the oldest kept if that is later), as many as one reply holds;
/// returns how many `visit` saw. Needs the read badge.
pub fn read(from: u64, mut visit: impl FnMut(&Entry)) -> crate::sys::Result<usize> {
    let endpoint = endpoint().ok_or(crate::sys::Error::NotFound)?;
    let c = client();
    if c.shared.is_none() { c.shared = Some(wire::Shared::new(4096)?); }
    let shared = c.shared.as_mut().unwrap();
    let list = log::read(endpoint, shared.buffer(), from)?.map_err(|_| crate::sys::Error::Rights)?;
    let mut count = 0;
    for entry in list.iter() { visit(&entry); count += 1; }
    Ok(count)
}

/// The state of the ring (oldest and next sequence numbers, dropped and refused records). Needs the read badge.
pub fn state() -> crate::sys::Result<State> {
    let endpoint = endpoint().ok_or(crate::sys::Error::NotFound)?;
    let c = client();
    if c.shared.is_none() { c.shared = Some(wire::Shared::new(4096)?); }
    let shared = c.shared.as_mut().unwrap();
    log::state(endpoint, shared.buffer())?.map_err(|_| crate::sys::Error::Rights)
}
