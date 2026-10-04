#![no_std]
#![no_main]
// tts: speech synthesis in ring 3. Text (UTF-8) arrives in a page shared by the client; speech is synthesized
// by a 16 kHz formant synthesizer, upsampled to 48 kHz stereo and streamed to audio_gw.
mod dsp;
mod phonemes;
mod synth;
mod text;

use mind::abi::*;
use mind::audio::Stream;
use mind::idl::{tts, wire};
use mind::ipc::Endpoint;
use mind::sys::Error;
use phonemes::{Ph, Unit};

const RECEIVED_CAP: usize = 9;
const MAX_UNITS: usize = 2048;

// Upsampling 16 -> 48 kHz by linear interpolation, mono -> stereo.
struct Upsampler { previous: i32, buffer: [i16; synth::FRAME * 6], error: bool }

impl Upsampler {
    fn push(&mut self, stream: &mut Stream, input: &[i16]) {
        let mut at = 0;
        for &sample in input {
            let (a, b) = (self.previous, sample as i32);
            for value in [(2 * a + b) / 3, (a + 2 * b) / 3, b] { self.buffer[at] = value as i16; self.buffer[at + 1] = value as i16; at += 2; }
            self.previous = b;
        }
        if stream.write(&self.buffer[..at]).is_err() { self.error = true; }
    }
}

fn say(text: &str, voice: synth::Voice) -> Result<usize, usize> {
    let mut units = [Unit { ph: Ph::Pause(0), soft: false, stress: false }; MAX_UNITS];
    let count = text::parse(text, &mut units);
    let mut stream = Stream::new().map_err(|e| e.code())?;
    let mut upsampler = Upsampler { previous: 0, buffer: [0; synth::FRAME * 6], error: false };
    let mut samples = 0usize;
    synth::speak(&units[..count], voice, &mut |chunk| { samples += chunk.len(); upsampler.push(&mut stream, chunk); });
    stream.flush().map_err(|e| e.code())?;
    if upsampler.error { return Err(ERR_PEER); }
    Ok(samples * 1000 / dsp::RATE as usize)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let present = mind::audio::info().map(|i| i.present).unwrap_or(false);
    mind::println!("[TTS] FORMANT SYNTHESIZER READY (RU/EN), AUDIO={}", present);
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        // idl/tts.wit: the text is copied into private memory and checked before synthesis.
        let (text, pitch, rate, call) = match tts::decode(&request, RECEIVED_CAP) {
            Ok((tts::Request::Say { text, pitch, rate }, call)) => (text, pitch, rate, call),
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let voice = synth::Voice { pitch: if pitch == 0 { 112 } else { pitch as i64 }, rate: if rate == 0 { 100 } else { rate as i64 } };
        let result = if !present { Err(ERR_NOT_FOUND) } else if text.as_str().is_empty() { Err(ERR_INVALID) } else { say(text.as_str(), voice) };
        match result { Ok(ms) => mind::println!("[TTS] SPOKE {} BYTES, {} MS", text.as_str().len(), ms), Err(code) => mind::println!("[TTS] ERROR {:#x}", code) }
        let _ = tts::reply_say(call, result.map(|ms| ms as u32).map_err(|code| mind::sys::check(code).err().unwrap_or(Error::Invalid)));
    }
}
