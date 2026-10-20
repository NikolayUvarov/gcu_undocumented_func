//! A copy of the tool itself that it starts and talks to: an echo for IPC (`--child`), the memory probes of `check`
//! (`--probe kernel|code|null`), which must be stopped by a fault, a reader of its text input (`--reader`), and one that
//! says which launch slots it holds (`--slots`).
use alloc::format;
use alloc::string::String;
use core::hint::black_box;
use mind::abi::{CAP_GRANT, CAP_KIND_ENDPOINT, CAP_READ, CAP_WRITE, LAUNCH_SLOTS, SLOT_CONSOLE, SLOT_INIT};
use mind::idl::loader;
use mind::io::Writer;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Mapping;

pub const HELLO: usize = 1;
pub const ECHO: usize = 2;
pub const EXIT: usize = 3;
/// A probe's access did not fault.
pub const SURVIVED: usize = 4;
/// The word at the start of a lent page, as the echo read it, in data[1] of its reply when data[0] asks for it.
pub const PEEK: usize = 5;
/// The reader's answer (`--reader`): data[0] = READ | how it ended << 8 (READ_* below), data[1] = the bytes it read;
/// then data[0] = HASH, data[1] = their FNV-1a hash.
pub const READ: usize = 6;
pub const HASH: usize = 7;
/// The `--slots` child's answer: data[1] has bit i set when `LAUNCH_SLOTS[i]` holds an endpoint.
pub const SLOTS: usize = 8;
/// The input ended, and a read after its end ended it again.
pub const READ_END: usize = 0;
/// The input ended, but a read after its end did not answer 0.
pub const READ_END_LOST: usize = 1;
/// A read failed.
pub const READ_ERROR: usize = 2;
const RECEIVED: usize = 9; // the child's fixed slot for a lent page (it has no console)

// A launch session of `program args` with `endpoint` (receive and send) granted in its SLOT_INIT.
fn session(program: &str, args: &str, endpoint: Endpoint) -> Result<u32, String> {
    let session = loader::begin(Endpoint::LOADER, program, args).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))?;
    let lent = ipc::mint(endpoint.0, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0).and_then(|client| {
        let granted = loader::grant(Endpoint::LOADER, session, SLOT_INIT as u8, client);
        let _ = ipc::drop_cap(client);
        granted?.map_err(|_| mind::Error::Invalid)
    });
    if let Err(e) = lent { let _ = loader::abort(Endpoint::LOADER, session); return Err(format!("grant: {:?}", e)); }
    Ok(session)
}

/// Starts `program args` holding `endpoint` (receive and send) in its SLOT_INIT; its PID.
pub fn start(program: &str, args: &str, endpoint: Endpoint) -> Result<u64, String> {
    let session = session(program, args, endpoint)?;
    loader::commit(Endpoint::LOADER, session).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))
}

/// `start` with a send-only copy of `endpoint` lent in every other launch slot as well, in one session (000-KRN-0101).
pub fn start_in_every_slot(program: &str, args: &str, endpoint: Endpoint) -> Result<u64, String> {
    let session = session(program, args, endpoint)?;
    for slot in LAUNCH_SLOTS.into_iter().filter(|&slot| slot != SLOT_INIT) {
        let granted = ipc::mint(endpoint.0, CAP_WRITE, 0, 0).map(|client| {
            let granted = loader::grant(Endpoint::LOADER, session, slot as u8, client);
            let _ = ipc::drop_cap(client);
            granted
        });
        if !matches!(granted, Ok(Ok(Ok(())))) { let _ = loader::abort(Endpoint::LOADER, session); return Err(format!("the grant in slot {}: {:?}", slot, granted)); }
    }
    loader::commit(Endpoint::LOADER, session).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))
}

/// Starts `program args` with a send-only copy of `endpoint` as its console (SLOT_CONSOLE), where its output comes.
pub fn start_with_console(program: &str, args: &str, endpoint: Endpoint) -> Result<u64, String> {
    let session = loader::begin(Endpoint::LOADER, program, args).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))?;
    let lent = ipc::mint(endpoint.0, CAP_WRITE, 0, 0).map(|client| {
        let granted = loader::grant(Endpoint::LOADER, session, SLOT_CONSOLE as u8, client);
        let _ = ipc::drop_cap(client);
        granted
    });
    if !matches!(lent, Ok(Ok(Ok(())))) { let _ = loader::abort(Endpoint::LOADER, session); return Err(format!("the console's grant: {:?}", lent)); }
    loader::commit(Endpoint::LOADER, session).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))
}

/// Says hello, then which launch slots hold an endpoint (`--slots`).
pub fn report_slots() {
    let parent = Endpoint::INIT;
    if parent.send_timeout(&Message::new(HELLO, 0), 10_000).is_err() { return; }
    let held = LAUNCH_SLOTS.iter().enumerate().filter(|&(_, &slot)| mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT).fold(0, |mask, (i, _)| mask | 1 << i);
    let _ = parent.send_timeout(&Message::new(SLOTS, held), 5000);
}

/// `start` with a text input (loader 1.9, 000-KRN-0094): its PID and the input's writer, which arrives in the fixed
/// slot `receive` and moves to a slot of its own.
pub fn start_fed(program: &str, args: &str, endpoint: Endpoint, receive: usize) -> Result<(u64, Writer), String> {
    let session = session(program, args, endpoint)?;
    loader::commit_with_input(Endpoint::LOADER, session, false, receive).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))?;
    let copy = ipc::mint(receive, u8::MAX, 0, 0);
    let _ = ipc::drop_cap(receive);
    let writer = copy.ok().and_then(Writer::new).ok_or_else(|| String::from("the loader gave no writer of a text input"))?;
    Ok((writer.reader(), writer))
}

/// FNV-1a over `bytes`, from `hash` (`FNV_START` first).
pub const FNV_START: u64 = 0xCBF2_9CE4_8422_2325;
pub fn fnv(hash: u64, bytes: &[u8]) -> u64 { bytes.iter().fold(hash, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01B3)) }

/// The reader: says hello, reads its text input to the end in pieces that cut across the writer's, then once more, and
/// tells its parent what it read and how it ended.
pub fn read_input() {
    let parent = Endpoint::INIT;
    if parent.send_timeout(&Message::new(HELLO, 0), 10_000).is_err() { return; }
    let (mut count, mut hash, mut buffer) = (0usize, FNV_START, [0u8; 1000]);
    let ended = loop {
        match mind::io::read(&mut buffer) {
            Ok(0) => break true,
            Ok(n) => { count += n; hash = fnv(hash, &buffer[..n]); }
            Err(_) => break false,
        }
    };
    let how = match (ended, mind::io::read(&mut buffer)) { (true, Ok(0)) => READ_END, (true, _) => READ_END_LOST, (false, _) => READ_ERROR };
    let _ = parent.send_timeout(&Message::new(READ | how << 8, count), 5000);
    let _ = parent.send_timeout(&Message::new(HASH, hash as usize), 5000);
}

/// Waits for a started child's hello.
pub fn hello(endpoint: Endpoint) -> Result<(), String> {
    match endpoint.recv_timeout(0, 10_000) {
        Ok(m) if m.data[0] == HELLO => Ok(()),
        Ok(m) => Err(format!("the child said {}", m.data[0])),
        Err(e) => Err(format!("no hello from the child: {:?}", e)),
    }
}

/// Whether `pid` ended within `ms`, polled every 10 ms.
pub fn gone_within(pid: u64, ms: u64) -> bool {
    let t = mind::time::monotonic_ns();
    while mind::process::alive(pid) && mind::time::monotonic_ns() - t < ms * 1_000_000 { mind::time::sleep(10); }
    !mind::process::alive(pid)
}

/// Tells the echo to exit and waits until it is gone (up to 2 s).
pub fn stop(endpoint: Endpoint, pid: u64) -> bool {
    let _ = endpoint.call_timeout(&Message::new(EXIT, 0), 0, 2000);
    gone_within(pid, 2000)
}

/// The echo: says hello, answers calls (mapping a lent page and reading its first word), exits when told or after 30 s
/// alone.
pub fn serve() {
    let parent = Endpoint::INIT;
    if parent.send_timeout(&Message::new(HELLO, 0), 10_000).is_err() { return; }
    while let Ok(request) = parent.recv_timeout(RECEIVED, 30_000) {
        let mut word = request.data[1];
        if request.cap_received {
            if let Ok(page) = Mapping::new(RECEIVED) { word = black_box(unsafe { core::ptr::read_volatile(page.address() as *const usize) }); }
            let _ = ipc::drop_cap(RECEIVED);
        }
        if request.is_call { let _ = ipc::reply(&Message::new(request.data[0], if request.data[0] == PEEK { word } else { request.data[1] })); }
        if request.data[0] == EXIT { return; }
    }
}

/// An address only the kernel may read: its identity map of low memory.
pub const KERNEL_ADDRESS: usize = if cfg!(target_arch = "aarch64") { 0x4000_0000 } else { 0x10_0000 };

/// A probe: says hello, makes an access the kernel must stop, and says it survived if it was not stopped.
pub fn probe(kind: &str) {
    let parent = Endpoint::INIT;
    if parent.send_timeout(&Message::new(HELLO, 0), 10_000).is_err() { return; }
    unsafe {
        match kind {
            "kernel" => { black_box(core::ptr::read_volatile(black_box(KERNEL_ADDRESS) as *const u64)); }
            "code" => { core::ptr::write_volatile(black_box(probe as *const () as usize) as *mut u8, 0); }
            _ => { black_box(core::ptr::read_volatile(black_box(0usize) as *const u64)); }
        }
    }
    let _ = parent.send_timeout(&Message::new(SURVIVED, 0), 2000);
}
