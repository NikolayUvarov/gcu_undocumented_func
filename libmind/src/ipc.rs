//! Точки IPC: рандеву send/recv, вызов call с ответом reply, передача мандатов и уведомления IRQ.
use crate::abi::*;
use crate::sys::{call, check, syscall, Result};

/// Исходящее сообщение: два слова данных и, по желанию, мандат из слота `cap` с маской прав.
#[derive(Clone, Copy, Debug, Default)]
pub struct Message { pub data: [usize; 2], pub cap: usize, pub rights: u8 }

impl Message {
    pub const fn new(a: usize, b: usize) -> Self { Self { data: [a, b], cap: 0, rights: 0 } }
    pub const fn with_cap(mut self, slot: usize, rights: u8) -> Self { self.cap = slot; self.rights = rights; self }
}

/// Принятое сообщение или ответ.
#[derive(Clone, Copy, Debug)]
pub struct Received { pub data: [usize; 2], pub sender: u64, pub cap_received: bool, pub is_call: bool, pub irq: Option<usize>, pub kernel: Option<usize> }

fn received(raw: crate::sys::Raw) -> Received {
    let irq = (raw.msg[1] & MSG_FLAG_IRQ != 0).then_some(raw.msg[2]);
    let kernel = (raw.msg[1] & MSG_FLAG_KERNEL != 0).then_some(raw.msg[2]);
    Received { data: [raw.msg[2], raw.msg[3]], sender: raw.arg1 as u64, cap_received: raw.msg[0] != 0, is_call: raw.msg[1] & MSG_FLAG_CALL != 0, irq, kernel }
}

/// Мандат точки IPC в слоте процесса.
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

    /// Новая точка со всеми правами.
    pub fn create() -> Result<Self> { check(call(SYSCALL_ENDPOINT_CREATE, 0, 0)).map(Self) }

    /// Блокирует до встречи с получателем; несколько отправителей встают в очередь.
    pub fn send(&self, message: &Message) -> Result<()> {
        check(syscall(SYSCALL_IPC_SEND, self.0, 0, [message.cap, message.rights as usize, message.data[0], message.data[1]]).result).map(drop)
    }

    /// Отправляет и ждёт ответа сервера; мандат из ответа попадёт в слот `receive` (0 — не принимать).
    pub fn call(&self, message: &Message, receive: usize) -> Result<Received> {
        let raw = syscall(SYSCALL_IPC_CALL, self.0, receive, [message.cap, message.rights as usize, message.data[0], message.data[1]]);
        check(raw.result).map(|_| received(raw))
    }

    /// Ждёт сообщение или уведомление IRQ; переданный мандат кладётся в слот `receive`.
    pub fn recv(&self, receive: usize) -> Result<Received> {
        let raw = syscall(SYSCALL_IPC_RECV, self.0, receive, [0; 4]);
        check(raw.result).map(|_| received(raw))
    }
}

/// Ответ клиенту последнего принятого `call`.
pub fn reply(message: &Message) -> Result<()> {
    check(syscall(SYSCALL_IPC_REPLY, 0, 0, [message.cap, message.rights as usize, message.data[0], message.data[1]]).result).map(drop)
}

/// Сохраняет право ответить последнему клиенту в слот мандата, чтобы ответить позже (`reply_saved`).
pub fn save_reply() -> Result<usize> { check(call(SYSCALL_IPC_SAVE_REPLY, 0, 0)) }

/// Ответ по сохранённому мандату; слот освобождается.
pub fn reply_saved(slot: usize, message: &Message) -> Result<()> {
    check(syscall(SYSCALL_IPC_REPLY, slot, 0, [message.cap, message.rights as usize, message.data[0], message.data[1]]).result).map(drop)
}

/// Освобождает слот мандата.
pub fn drop_cap(slot: usize) -> Result<()> { check(call(SYSCALL_CAP_DROP, slot, 0)).map(drop) }
