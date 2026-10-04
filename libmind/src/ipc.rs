//! IPC endpoints: send/recv rendezvous, call with reply, capability transfer and IRQ notifications.
use crate::abi::*;
use crate::sys::{call, check, syscall, Result};

/// Outgoing message: two data words and, optionally, the capability `cap` (a handle) with a rights mask; it is copied
/// (the receiver gets a child the sender can revoke) unless `moved`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Message { pub data: [usize; 2], pub cap: usize, pub rights: u8, pub moved: bool }

impl Message {
    pub const fn new(a: usize, b: usize) -> Self { Self { data: [a, b], cap: 0, rights: 0, moved: false } }
    pub const fn with_cap(mut self, slot: usize, rights: u8) -> Self { self.cap = slot; self.rights = rights; self }
    /// Moves the capability: the sender's handle becomes invalid once the message is delivered.
    pub const fn with_cap_moved(mut self, slot: usize, rights: u8) -> Self { self.cap = slot; self.rights = rights; self.moved = true; self }
    fn mask(&self) -> usize { self.rights as usize | if self.moved { CAP_TRANSFER_MOVE } else { 0 } }
}

/// Received message or reply.
#[derive(Clone, Copy, Debug)]
pub struct Received { pub data: [usize; 2], pub sender: u64, pub cap_received: bool, pub is_call: bool, pub irq: Option<usize> }

fn received(raw: crate::sys::Raw) -> Received {
    let irq = (raw.msg[1] & MSG_FLAG_IRQ != 0).then_some(raw.msg[2]);
    Received { data: [raw.msg[2], raw.msg[3]], sender: raw.arg1 as u64, cap_received: raw.msg[0] != 0, is_call: raw.msg[1] & MSG_FLAG_CALL != 0, irq }
}

/// IPC endpoint capability in a process slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint(pub usize);

impl Endpoint {
    pub const RTC: Self = Self(SLOT_RTC);
    pub const VFS: Self = Self(SLOT_VFS);
    pub const AUDIO: Self = Self(SLOT_AUDIO);
    pub const LOADER: Self = Self(SLOT_LOADER);
    pub const TTS: Self = Self(SLOT_TTS);
    pub const INIT: Self = Self(SLOT_INIT);
    pub const SERVICE: Self = Self(SLOT_SERVICE);
    /// sysmon client (idl/sysinfo.wit), when the launcher granted it.
    pub const SYSINFO: Self = Self(SLOT_SYSINFO);

    /// New endpoint with all rights.
    pub fn create() -> Result<Self> { check(call(SYSCALL_ENDPOINT_CREATE, 0, 0)).map(Self) }

    /// Blocks until a receiver arrives; multiple senders are queued.
    pub fn send(&self, message: &Message) -> Result<()> { self.send_timeout(message, 0) }

    /// `send` that fails with `ERR_TIMEOUT` after `ms` milliseconds (0: no limit).
    pub fn send_timeout(&self, message: &Message, ms: u32) -> Result<()> {
        check(queued(SYSCALL_IPC_SEND, self.word(ms), 0, message).result).map(drop)
    }

    /// Sends and waits for the server's reply; a capability in the reply lands in slot `receive` (0 means don't accept).
    pub fn call(&self, message: &Message, receive: usize) -> Result<Received> { self.call_timeout(message, receive, 0) }

    /// `call` that gives up after `ms` milliseconds (0: no limit); a later reply from the server is discarded.
    pub fn call_timeout(&self, message: &Message, receive: usize, ms: u32) -> Result<Received> {
        let raw = queued(SYSCALL_IPC_CALL, self.word(ms), receive, message);
        check(raw.result).map(|_| received(raw))
    }

    /// Waits for a message or IRQ notification; a transferred capability is placed in slot `receive`.
    pub fn recv(&self, receive: usize) -> Result<Received> { self.recv_timeout(receive, 0) }

    /// `recv` that fails with `ERR_TIMEOUT` after `ms` milliseconds (0: no limit).
    pub fn recv_timeout(&self, receive: usize, ms: u32) -> Result<Received> {
        let raw = syscall(SYSCALL_IPC_RECV, self.word(ms), receive, [0; 4]);
        check(raw.result).map(|_| received(raw))
    }

    fn word(&self, ms: u32) -> usize { self.0 | (ms as usize) << IPC_TIMEOUT_SHIFT }
}

// A full endpoint queue (ERR_BUSY) is back-pressure: wait a tick and try again.
fn queued(number: usize, word: usize, receive: usize, message: &Message) -> crate::sys::Raw {
    loop {
        let raw = syscall(number, word, receive, [message.cap, message.mask(), message.data[0], message.data[1]]);
        if raw.result != ERR_BUSY { return raw; }
        call(SYSCALL_WAIT, 10, 0);
    }
}

/// Replies to the client of the last received `call`.
pub fn reply(message: &Message) -> Result<()> {
    check(syscall(SYSCALL_IPC_REPLY, 0, 0, [message.cap, message.mask(), message.data[0], message.data[1]]).result).map(drop)
}

/// Saves the right to reply to the last client into a capability slot, to reply later (`reply_saved`).
pub fn save_reply() -> Result<usize> { check(call(SYSCALL_IPC_SAVE_REPLY, 0, 0)) }

/// Replies via a saved capability; the slot is freed.
pub fn reply_saved(slot: usize, message: &Message) -> Result<()> {
    check(syscall(SYSCALL_IPC_REPLY, slot, 0, [message.cap, message.mask(), message.data[0], message.data[1]]).result).map(drop)
}

/// Frees a capability slot.
pub fn drop_cap(slot: usize) -> Result<()> { check(call(SYSCALL_CAP_DROP, slot, 0)).map(drop) }

/// Child capability with narrower authority: endpoint rights `mask`, or a sub-range (`offset`, `length`, 0 = to the end)
/// of a port range or a page-aligned memory/DMA/MMIO range. Returns its handle.
pub fn mint(handle: usize, mask: u8, offset: usize, length: usize) -> Result<usize> {
    check(syscall(SYSCALL_CAP_MINT, handle, mask as usize, [offset, length, 0, 0]).result)
}

/// Removes every capability derived from `handle` (copies, mints and their descendants) from all tasks; the capability
/// itself stays. Returns the number removed; when it returns, none of them can be used any more.
pub fn revoke(handle: usize) -> Result<usize> { check(call(SYSCALL_CAP_REVOKE, handle, 0)) }
