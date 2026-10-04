// Shared by `wintest` and `winmgr` (issue 157): the broker client and leases of window surfaces.
use mind::abi::SLOT_WINDOW;
use mind::idl::window as api;
use mind::ipc::Endpoint;
use mind::mem::Mapping;
use mind::util::FixedBuf;
use mind::window::Surface;

pub const BROKER: Endpoint = Endpoint(SLOT_WINDOW);
pub const RECEIVE: usize = 9;

pub fn text(buf: &FixedBuf<64>) -> &str { core::str::from_utf8(buf.as_bytes()).unwrap_or("") }

/// The lease of window `id` mapped (the program's own, or the manager's).
pub fn map(id: u32) -> Option<(Mapping, Surface)> {
    api::surface(BROKER, id, RECEIVE).ok()?.ok()?;
    let mapping = Mapping::new(RECEIVE).ok()?;
    let _ = mind::ipc::drop_cap(RECEIVE); // the mapping keeps the memory
    let surface = unsafe { Surface::new(mapping.as_ptr::<u8>(), mapping.len()) };
    Some((mapping, surface))
}
