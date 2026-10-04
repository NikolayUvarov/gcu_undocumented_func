//! Client for the audio_gw audio gateway (idl/audio.wit): 16-bit stereo 48 kHz PCM via a lent buffer, tones,
//! microphone capture.
use crate::abi::*;
use crate::idl::audio as idl;
use crate::ipc::Endpoint;
use crate::mem::Pages;
use crate::sys::{Error, Result};
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

#[derive(Clone, Copy, Debug)]
pub struct Info { pub present: bool, pub rate: usize }

pub fn info() -> Result<Info> { let rate = idl::device(Endpoint::AUDIO)?; Ok(Info { present: rate.is_some(), rate: rate.unwrap_or(AUDIO_RATE as u32) as usize }) }

/// Queues a sine wave of `hz` lasting `ms`.
pub fn tone(hz: usize, ms: usize) -> Result<()> { idl::tone(Endpoint::AUDIO, hz as u32, ms as u32).map(drop) }

/// Flushes the playback queue.
pub fn stop() -> Result<()> { idl::stop(Endpoint::AUDIO) }

/// Submits part of the interleaved L/R samples; returns the number of samples accepted (0 means the queue is full).
pub fn play(samples: &[i16]) -> Result<usize> {
    let channel = channel()?;
    let count = samples.len().min(CHUNK / 2) & !1;
    let bytes = channel.pages.as_mut_slice();
    for (i, sample) in samples[..count].iter().enumerate() { bytes[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes()); }
    let accepted = idl::play(Endpoint::AUDIO, (count * 2) as u32, channel.cap)? as usize;
    Ok(accepted / 2)
}

/// Waits until `buffers` 4 KiB buffers are free in the gateway's DMA ring (the reply comes on the AC97 interrupt).
pub fn wait_space(buffers: usize) -> Result<usize> { idl::wait(Endpoint::AUDIO, buffers.min(255) as u8).map(|free| free as usize) }

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
            let accepted = idl::play(Endpoint::AUDIO, ((self.filled & !1) * 2) as u32, channel.cap)? as usize;
            let taken = (accepted / 2).min(self.filled);
            if taken == 0 { if wait_space(CHUNK / 4096).is_err() { crate::time::sleep(20); } continue; }
            channel.pages.as_mut_slice().copy_within(taken * 2..self.filled * 2, 0);
            self.filled -= taken;
        }
        self.filled = 0;
        Ok(())
    }
}

/// Starts microphone capture (48 kHz stereo); Err(NotFound) without a capture-capable device.
pub fn record_start() -> Result<()> { idl::record_start(Endpoint::AUDIO) }

/// Stops microphone capture.
pub fn record_stop() -> Result<()> { idl::record_stop(Endpoint::AUDIO) }

/// Copies captured interleaved L/R samples into `out`; returns (samples, overflow). 0 samples means nothing new yet.
pub fn record_read(out: &mut [i16]) -> Result<(usize, bool)> {
    let channel = channel()?;
    let capacity = (out.len() * 2).min(CHUNK) & !4095;
    if capacity == 0 { return Ok((0, false)); }
    let before = idl::overflows(Endpoint::AUDIO)?;
    let bytes = idl::record_read(Endpoint::AUDIO, capacity as u32, channel.cap)? as usize;
    let overflow = idl::overflows(Endpoint::AUDIO)? != before;
    let samples = (bytes / 2).min(out.len());
    let data = channel.pages.as_slice();
    for (i, sample) in out[..samples].iter_mut().enumerate() { *sample = i16::from_le_bytes([data[i * 2], data[i * 2 + 1]]); }
    Ok((samples, overflow))
}
