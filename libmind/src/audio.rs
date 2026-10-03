//! Client for the audio_gw audio gateway: 16-bit stereo 48 kHz PCM via a shared buffer, tones.
use crate::abi::*;
use crate::ipc::{Endpoint, Message};
use crate::mem::Pages;
use crate::sys::{check, Error, Result};
use core::cell::UnsafeCell;

const CHUNK: usize = 16 * 1024;

struct Channel { pages: Pages, cap: usize }
struct Shared(UnsafeCell<Option<Channel>>);
unsafe impl Sync for Shared {} // processes are single-threaded
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

/// Queues a sine wave of `hz` lasting `ms`.
pub fn tone(hz: usize, ms: usize) -> Result<()> { request(Message::new(AUDIO_TONE | hz << 8, ms)).map(drop) }

/// Flushes the playback queue.
pub fn stop() -> Result<()> { request(Message::new(AUDIO_STOP, 0)).map(drop) }

/// Submits part of the interleaved L/R samples; returns the number of samples accepted (0 means the queue is full).
pub fn play(samples: &[i16]) -> Result<usize> {
    let channel = channel()?;
    let count = samples.len().min(CHUNK / 2) & !1;
    let bytes = channel.pages.as_mut_slice();
    for (i, sample) in samples[..count].iter().enumerate() { bytes[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes()); }
    let [accepted, _] = request(Message::new(AUDIO_PLAY | (count * 2) << 8, 0).with_cap(channel.cap, 0))?;
    Ok(accepted / 2)
}

/// Waits until `buffers` 4 KiB buffers are free in the gateway's DMA ring (the reply comes on the AC97 interrupt).
pub fn wait_space(buffers: usize) -> Result<usize> { request(Message::new(AUDIO_WAIT | buffers << 8, 0)).map(|[free, _]| free) }

/// Plays the whole buffer, waiting for queue space via gateway notifications.
pub fn play_all(mut samples: &[i16]) -> Result<()> {
    while samples.len() >= 2 {
        let accepted = play(samples)?;
        if accepted == 0 && wait_space(CHUNK / 4096).is_err() { crate::time::sleep(20); }
        samples = &samples[accepted..];
    }
    Ok(())
}

/// Streaming output: samples are copied directly into the shared buffer and sent to the gateway in 16 KiB blocks
/// while the producer (e.g. a speech synthesizer) prepares the next ones.
pub struct Stream { filled: usize }

impl Stream {
    pub fn new() -> Result<Self> { channel()?; Ok(Self { filled: 0 }) }
    /// Appends interleaved L/R samples.
    pub fn write(&mut self, samples: &[i16]) -> Result<()> {
        for sample in samples {
            if self.filled == CHUNK / 2 { self.flush()?; }
            let bytes = channel()?.pages.as_mut_slice();
            bytes[self.filled * 2..self.filled * 2 + 2].copy_from_slice(&sample.to_le_bytes());
            self.filled += 1;
        }
        Ok(())
    }
    /// Hands everything accumulated to the gateway, waiting for space in the ring.
    pub fn flush(&mut self) -> Result<()> {
        while self.filled >= 2 {
            let channel = channel()?;
            let [accepted, _] = request(Message::new(AUDIO_PLAY | (self.filled & !1) * 2 << 8, 0).with_cap(channel.cap, 0))?;
            let taken = (accepted / 2).min(self.filled);
            if taken == 0 { if wait_space(CHUNK / 4096).is_err() { crate::time::sleep(20); } continue; }
            channel.pages.as_mut_slice().copy_within(taken * 2..self.filled * 2, 0);
            self.filled -= taken;
        }
        self.filled = 0;
        Ok(())
    }
}
