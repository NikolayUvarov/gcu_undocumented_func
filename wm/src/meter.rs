//! The Sound page's microphone meter (000-APP-0077): while the page shows it, wm records through its own audio client,
//! which may record but not set the gain, and shows each half second's peak and RMS and the noise floor, as `hear`
//! measures them (000-APP-0053). The capture stops when the meter is dropped: the page left, or wm ending.

use alloc::string::String;
use alloc::vec::Vec;
use mind::voice::{meter, Detector};

// The gateway's 48 kHz stereo kept as 16 kHz mono, the detector's rate: the left sample of every third frame.
const KEEP_EVERY: usize = 3;
// One reading: half a second of 16 kHz, as often as the page is drawn again for it.
const READING: usize = 8000;
// Reads of the gateway's buffers in one call, so a call stays short.
const READS: usize = 8;

pub struct Meter { detector: Detector, raw: Vec<i16>, mono: Vec<i16>, frame: usize }

impl Meter {
    /// Starts the capture, or says why the microphone cannot be read.
    pub fn start() -> Result<Meter, String> {
        match mind::audio::record_start() {
            Ok(()) => Ok(Meter { detector: Detector::new(), raw: alloc::vec![0; 4096], mono: Vec::new(), frame: 0 }),
            Err(mind::sys::Error::NotFound) => Err(String::from("No microphone to read.")),
            Err(mind::sys::Error::Other(code)) if code == mind::abi::ERR_BUSY => Err(String::from("Another program records: the meter waits for the microphone.")),
            Err(error) => Err(alloc::format!("The microphone cannot be read ({:?}).", error)),
        }
    }

    /// What the microphone gave since the last call: the last whole half second's peak, RMS and noise floor in dBFS.
    pub fn poll(&mut self) -> Option<(i32, i32, i32)> {
        let mut level = None;
        for _ in 0..READS {
            let count = match mind::audio::record_read(&mut self.raw) { Ok((count, _)) if count > 0 => count, _ => break };
            for pair in self.raw[..count].chunks_exact(2) {
                if self.frame % KEEP_EVERY == 0 { self.mono.push(pair[0]); }
                self.frame += 1;
            }
            while self.mono.len() >= READING {
                let part: Vec<i16> = self.mono.drain(..READING).collect();
                self.detector.push(&part, &mut |_| {});
                let (peak, rms) = meter(&part);
                level = Some((peak, rms, self.detector.floor_dbfs()));
            }
        }
        level
    }
}

impl Drop for Meter {
    fn drop(&mut self) { let _ = mind::audio::record_stop(); }
}
