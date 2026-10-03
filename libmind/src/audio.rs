//! Клиент аудиошлюза audio_gw: PCM 16 бит стерео 48 кГц через разделяемый буфер, тоны.
use crate::abi::*;
use crate::ipc::{Endpoint, Message};
use crate::mem::Pages;
use crate::sys::{check, Error, Result};
use core::cell::UnsafeCell;

const CHUNK: usize = 16 * 1024;

struct Channel { pages: Pages, cap: usize }
struct Shared(UnsafeCell<Option<Channel>>);
unsafe impl Sync for Shared {} // процессы однопоточны
static CHANNEL: Shared = Shared(UnsafeCell::new(None));

fn channel() -> Result<&'static mut Channel> {
    let slot = unsafe { &mut *CHANNEL.0.get() };
    if slot.is_none() { let pages = Pages::new(CHUNK).ok_or(Error::NoMemory)?; let cap = pages.share()?; *slot = Some(Channel { pages, cap }); }
    Ok(slot.as_mut().unwrap())
}

fn request(message: Message) -> Result<[usize; 2]> {
    let reply = Endpoint::AUDIO.call(&message, 0)?;
    check(reply.data[0])?;
    Ok(reply.data)
}

#[derive(Clone, Copy, Debug)]
pub struct Info { pub present: bool, pub rate: usize }

pub fn info() -> Result<Info> { let [present, rate] = request(Message::new(AUDIO_INFO, 0))?; Ok(Info { present: present != 0, rate }) }

/// Ставит в очередь синусоиду `hz` длительностью `ms`.
pub fn tone(hz: usize, ms: usize) -> Result<()> { request(Message::new(AUDIO_TONE | hz << 8, ms)).map(drop) }

/// Сбрасывает очередь воспроизведения.
pub fn stop() -> Result<()> { request(Message::new(AUDIO_STOP, 0)).map(drop) }

/// Отдаёт часть чередующихся сэмплов L/R; возвращает число принятых сэмплов (0 — очередь полна).
pub fn play(samples: &[i16]) -> Result<usize> {
    let channel = channel()?;
    let count = samples.len().min(CHUNK / 2) & !1;
    let bytes = channel.pages.as_mut_slice();
    for (i, sample) in samples[..count].iter().enumerate() { bytes[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes()); }
    let [accepted, _] = request(Message::new(AUDIO_PLAY | (count * 2) << 8, 0).with_cap(channel.cap, 0))?;
    Ok(accepted / 2)
}

/// Проигрывает весь буфер, дожидаясь места в очереди.
pub fn play_all(mut samples: &[i16]) -> Result<()> {
    while samples.len() >= 2 {
        let accepted = play(samples)?;
        if accepted == 0 { crate::time::sleep(20); }
        samples = &samples[accepted..];
    }
    Ok(())
}
