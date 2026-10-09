//! Voice front end (docs/voice, V0): sound from the microphone or a WAV file as a 16 kHz mono stream cut into
//! utterances. `Stream::new(Microphone::start()?)` or `Stream::new(Wav::open(path)?)` gives 16 kHz mono samples;
//! `Detector` finds the speech in them.
mod front;
pub mod fbank;
pub mod features;
pub mod math;
pub mod grammar;
pub mod model;
pub mod recognizer;

pub use front::*;

use crate::sys::{Error, Result};
use alloc::vec;

/// WAV files larger than this are refused (they are read whole into the heap).
pub const MAX_WAV: usize = 8 << 20;

/// The microphone through the audio gateway: 48 kHz stereo; capture stops when it is dropped.
pub struct Microphone { overflows: usize }

impl Microphone {
    /// Starts capture; `Err(NotFound)` without a capture-capable device.
    pub fn start() -> Result<Self> { crate::audio::record_start()?; Ok(Self { overflows: 0 }) }
    /// Reads in which the gateway's capture ring had overflowed (samples were lost).
    pub fn overflows(&self) -> usize { self.overflows }
}

impl Drop for Microphone { fn drop(&mut self) { let _ = crate::audio::record_stop(); } }

impl Source for Microphone {
    type Error = Error;
    fn rate(&self) -> u32 { crate::abi::AUDIO_RATE as u32 }
    fn channels(&self) -> usize { 2 }
    fn read(&mut self, out: &mut [i16]) -> Result<usize> {
        let (count, overflow) = crate::audio::record_read(out)?;
        self.overflows += overflow as usize;
        Ok(count & !1)
    }
    fn finished(&self) -> bool { false }
}

/// Why `Wav::open` failed: the file could not be read, or it is not a WAV `Wav` reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError { File(crate::fs::Error), TooLarge, Format(WavError) }

impl Wav {
    /// Reads a WAV file (at most `MAX_WAV` bytes) through the file client.
    pub fn open(path: &str) -> core::result::Result<Self, OpenError> {
        let mut file = crate::fs::File::open(path).map_err(OpenError::File)?;
        if file.size() > MAX_WAV { return Err(OpenError::TooLarge); }
        let mut bytes = vec![0u8; file.size()];
        let mut filled = 0;
        while filled < bytes.len() {
            let got = file.read(&mut bytes[filled..]).map_err(OpenError::File)?;
            if got == 0 { break; }
            filled += got;
        }
        bytes.truncate(filled);
        Wav::parse(bytes).map_err(OpenError::Format)
    }
}
