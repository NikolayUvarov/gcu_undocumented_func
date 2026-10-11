//! A copy of the tool itself that it starts and talks to: an echo for IPC (`--child`) and the memory probes of `check`
//! (`--probe kernel|code|null`), which must be stopped by a fault.
use alloc::format;
use alloc::string::String;
use core::hint::black_box;
use mind::abi::{CAP_GRANT, CAP_READ, CAP_WRITE, SLOT_INIT};
use mind::idl::loader;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Mapping;

pub const HELLO: usize = 1;
pub const ECHO: usize = 2;
pub const EXIT: usize = 3;
/// A probe's access did not fault.
pub const SURVIVED: usize = 4;
/// The word at the start of a lent page, as the echo read it, in data[1] of its reply when data[0] asks for it.
pub const PEEK: usize = 5;
const RECEIVED: usize = 9; // the child's fixed slot for a lent page (it has no console)

/// Starts `program args` holding `endpoint` (receive and send) in its SLOT_INIT; its PID.
pub fn start(program: &str, args: &str, endpoint: Endpoint) -> Result<u64, String> {
    let session = loader::begin(Endpoint::LOADER, program, args).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))?;
    let lent = ipc::mint(endpoint.0, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0).and_then(|client| {
        let granted = loader::grant(Endpoint::LOADER, session, SLOT_INIT as u8, client);
        let _ = ipc::drop_cap(client);
        granted?.map_err(|_| mind::Error::Invalid)
    });
    if let Err(e) = lent { let _ = loader::abort(Endpoint::LOADER, session); return Err(format!("grant: {:?}", e)); }
    loader::commit(Endpoint::LOADER, session).map_err(|e| format!("loader: {:?}", e))?.map_err(|e| format!("loader: {:?}", e))
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
