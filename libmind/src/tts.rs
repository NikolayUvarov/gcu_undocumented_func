//! Клиент сервиса tts: синтез речи (русский и латиница) через audio_gw.
use crate::abi::*;
use crate::ipc::{Endpoint, Message};
use crate::mem::Pages;
use crate::sys::{check, Error, Result};
use core::cell::UnsafeCell;

struct Channel { pages: Pages, cap: usize }
struct Shared(UnsafeCell<Option<Channel>>);
unsafe impl Sync for Shared {} // процессы однопоточны
static CHANNEL: Shared = Shared(UnsafeCell::new(None));

fn channel() -> Result<&'static mut Channel> {
    let slot = unsafe { &mut *CHANNEL.0.get() };
    if slot.is_none() { let pages = Pages::new(4096).ok_or(Error::NoMemory)?; let cap = pages.share()?; *slot = Some(Channel { pages, cap }); }
    Ok(slot.as_mut().unwrap())
}

/// Произносит текст (до 4 КиБ UTF-8) голосом по умолчанию; возвращает длительность речи в мс, когда она поставлена в очередь.
pub fn say(text: &str) -> Result<usize> { say_with(text, 0, 0) }

/// То же с высотой тона (Гц, 0 — 112) и темпом (%, 0 — 100).
pub fn say_with(text: &str, pitch: u16, rate: u16) -> Result<usize> {
    if text.len() > 4096 { return Err(Error::Invalid); }
    let channel = channel()?;
    channel.pages.as_mut_slice()[..text.len()].copy_from_slice(text.as_bytes());
    let reply = Endpoint::TTS.call(&Message::new(TTS_SAY | text.len() << 8, pitch as usize | (rate as usize) << 16).with_cap(channel.cap, 0), 0)?;
    check(reply.data[0])
}
