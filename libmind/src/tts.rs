//! Client for the tts service (idl/tts.wit): speech synthesis (Russian and Latin script) via audio_gw.
use crate::ipc::Endpoint;
use crate::sys::{Error, Result};

/// Speaks text (up to 4000 bytes of UTF-8) in the default voice; returns the speech duration in ms once it is queued.
pub fn say(text: &str) -> Result<usize> { say_with(text, 0, 0) }

/// Same, with pitch (Hz, 0 = 112) and tempo (%, 0 = 100).
pub fn say_with(text: &str, pitch: u16, rate: u16) -> Result<usize> {
    if text.len() > 4000 { return Err(Error::Invalid); }
    crate::idl::tts::say(Endpoint::TTS, text, pitch, rate).map(|ms| ms as usize)
}
