//! What a program prints, also sent to the program that started it (issue 162): when its launcher lent an endpoint in
//! `SLOT_CONSOLE` (`console` does), `mind::process::log` sends each piece of output there as well as to the kernel's
//! log, 15 bytes a message — the length in the first byte, then the bytes. Whether the slot holds an endpoint is looked
//! at once, before `main` (a service may later receive capabilities in slot 9). `pack` and `unpack` make no system
//! calls: tests/console_host.rs.
pub const CHUNK: usize = 15;

/// The message words for up to `CHUNK` bytes.
pub fn pack(bytes: &[u8]) -> [usize; 2] {
    let len = bytes.len().min(CHUNK);
    let mut raw = [0u8; 16];
    raw[0] = len as u8;
    raw[1..1 + len].copy_from_slice(&bytes[..len]);
    [usize::from_le_bytes(raw[..8].try_into().unwrap()), usize::from_le_bytes(raw[8..].try_into().unwrap())]
}

/// The bytes of a message, into `out`; their count.
pub fn unpack(data: [usize; 2], out: &mut [u8; CHUNK]) -> usize {
    let mut raw = [0u8; 16];
    raw[..8].copy_from_slice(&data[0].to_le_bytes());
    raw[8..].copy_from_slice(&data[1].to_le_bytes());
    let len = (raw[0] as usize).min(CHUNK);
    out[..len].copy_from_slice(&raw[1..1 + len]);
    len
}

#[cfg(target_os = "none")]
mod system {
    use crate::abi::{CAP_KIND_ENDPOINT, SLOT_CONSOLE};
    use crate::ipc::{Endpoint, Message};
    use core::sync::atomic::{AtomicBool, Ordering};

    static OPEN: AtomicBool = AtomicBool::new(false);

    /// Before `main` (`mind::entry!`): whether the launcher lent an endpoint for the output.
    pub fn prepare() { OPEN.store(crate::dev::cap_info(SLOT_CONSOLE).0 == CAP_KIND_ENDPOINT, Ordering::Relaxed); }

    /// Sends `bytes` to the launcher's endpoint, if there is one. A launcher that does not take a message within half
    /// a second loses the rest of this piece; one that ended gets no more.
    pub fn send(bytes: &[u8]) {
        if !OPEN.load(Ordering::Relaxed) { return; }
        for chunk in bytes.chunks(super::CHUNK) {
            let [a, b] = super::pack(chunk);
            match Endpoint(SLOT_CONSOLE).send_timeout(&Message::new(a, b), 500) {
                Ok(()) => {}
                Err(crate::sys::Error::Peer) | Err(crate::sys::Error::Rights) | Err(crate::sys::Error::NotFound) => { OPEN.store(false, Ordering::Relaxed); return; }
                Err(_) => return,
            }
        }
    }
}
#[cfg(target_os = "none")]
pub use system::{prepare, send};
