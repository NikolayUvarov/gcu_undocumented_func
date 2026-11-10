#![no_std]
#![no_main]
// tts: speech synthesis in ring 3. Text (UTF-8) arrives in a page shared by the client; speech is synthesized
// by a 16 kHz formant synthesizer, upsampled to 48 kHz stereo (a windowed-sinc filter) and streamed to audio_gw.
// Russian sentences take the neural voice from the model disk where init lends it (252-APP-0128).
extern crate alloc;

mod dsp;
mod neural;
mod route;
mod synth;

// Text to phonemes is shared with the voice recognizer (issue 078).
use phonetics::{phonemes, text};

use mind::abi::*;
use mind::audio::Stream;
use mind::idl::{tts, wire};
use mind::ipc::Endpoint;
use mind::sys::Error;
use phonemes::{Ph, Unit};

const RECEIVED_CAP: usize = 9;
const MAX_UNITS: usize = 2048;

// 16 -> 48 kHz through a windowed-sinc filter (linear interpolation left the spectrum's images 20-30 dB below the
// speech and dulled 6 kHz by 4 dB, 252-APP-0041), mono -> stereo.
struct Upsampler { filter: dsp::Upsampler, buffer: [i16; synth::FRAME * 2 * dsp::UP], error: bool }

impl Upsampler {
    fn push(&mut self, stream: &mut Stream, input: &[i16]) {
        let mut at = 0;
        for &sample in input {
            for value in self.filter.run(sample) { self.buffer[at] = value; self.buffer[at + 1] = value; at += 2; }
        }
        if stream.write(&self.buffer[..at]).is_err() { self.error = true; }
    }
}

// The text, or Russian sentences through the neural voice where it is (252-APP-0128): its duration in ms, and how long
// after the request the neural voice's first sound was handed to the audio stream (its load included, the first time).
fn say(text: &str, voice: synth::Voice, neural: &mut neural::Neural) -> Result<(usize, Option<usize>), usize> {
    let started = mind::time::uptime_ms();
    let mut stream = Stream::new().map_err(|e| e.code())?;
    let Some(nv) = (if route::russian(text) { neural.voice() } else { None }) else {
        let ms = formant(text, voice, &mut stream)?;
        stream.flush().map_err(|e| e.code())?;
        return Ok((ms, None));
    };
    let (mut ms, mut first) = (0, None);
    for (sentence, russian) in neural::sentences(text) {
        let mut mono = alloc::vec::Vec::new();
        match russian.then(|| neural::say(nv, sentence, voice.rate, &mut mono)) {
            Some(Ok(())) => {
                ms += mono.len() * 1000 / AUDIO_RATE;
                let stereo: alloc::vec::Vec<i16> = mono.iter().flat_map(|&s| [s, s]).collect();
                stream.write(&stereo).map_err(|e| e.code())?;
                first.get_or_insert(mind::time::uptime_ms() - started);
            }
            Some(Err(why)) => { mind::println!("[TTS] THE NEURAL VOICE FAILED ({}): THE FORMANT VOICE SAYS THE SENTENCE", why); ms += formant(sentence, voice, &mut stream)?; }
            None => ms += formant(sentence, voice, &mut stream)?,
        }
    }
    stream.flush().map_err(|e| e.code())?;
    Ok((ms, first))
}

// The formant voice: its duration in ms.
fn formant(text: &str, voice: synth::Voice, stream: &mut Stream) -> Result<usize, usize> {
    let mut units = [Unit { ph: Ph::Pause(0), soft: false, stress: false }; MAX_UNITS];
    let count = text::parse(text, &mut units);
    let mut upsampler = Upsampler { filter: dsp::Upsampler::new(), buffer: [0; synth::FRAME * 2 * dsp::UP], error: false };
    let mut samples = 0usize;
    synth::speak(&units[..count], voice, &mut |chunk| { samples += chunk.len(); upsampler.push(stream, chunk); });
    upsampler.push(stream, &[0; dsp::TAPS / 2]); // the filter's delay: the last millisecond out
    if upsampler.error { return Err(ERR_PEER); }
    Ok(samples * 1000 / dsp::RATE as usize)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let present = mind::audio::info().map(|i| i.present).unwrap_or(false);
    mind::println!("[TTS] FORMANT SYNTHESIZER READY (RU/EN), AUDIO={}", present);
    let mut neural = neural::Neural::Untried;
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        // idl/tts.wit: the text is copied into private memory and checked before synthesis.
        let (text, pitch, rate, call) = match tts::decode(&request, RECEIVED_CAP) {
            Ok((tts::Request::Say { text, pitch, rate }, call)) => (text, pitch, rate, call),
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let voice = synth::Voice { pitch: if pitch == 0 { 112 } else { pitch as i64 }, rate: if rate == 0 { 100 } else { rate as i64 } };
        let result = if !present { Err(ERR_NOT_FOUND) } else if text.as_str().is_empty() { Err(ERR_INVALID) } else { say(text.as_str(), voice, &mut neural) };
        match result {
            Ok((ms, Some(first))) => mind::println!("[TTS] SPOKE {} BYTES, {} MS; THE NEURAL VOICE'S FIRST SOUND AFTER {} MS", text.as_str().len(), ms, first),
            Ok((ms, None)) => mind::println!("[TTS] SPOKE {} BYTES, {} MS", text.as_str().len(), ms),
            Err(code) => mind::println!("[TTS] ERROR {:#x}", code),
        }
        let _ = tts::reply_say(call, result.map(|(ms, _)| ms as u32).map_err(|code| mind::sys::check(code).err().unwrap_or(Error::Invalid)));
    }
}
