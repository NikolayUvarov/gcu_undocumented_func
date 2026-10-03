#![no_std]
#![no_main]
// tts: синтез речи в ring 3. Текст (UTF-8) приходит в разделяемой странице клиента, речь синтезируется
// формантным синтезатором 16 кГц, повышается до 48 кГц стерео и потоком уходит в audio_gw.
mod dsp;
mod phonemes;
mod synth;
mod text;

use mind::abi::*;
use mind::audio::Stream;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Mapping;
use phonemes::{Ph, Unit};

const RECEIVED_CAP: usize = 9;
const MAX_UNITS: usize = 2048;

// Повышение частоты 16 -> 48 кГц линейной интерполяцией, моно -> стерео.
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
        let (op, len) = (request.data[0] & 0xFF, request.data[0] >> 8);
        let voice = synth::Voice { pitch: match request.data[1] & 0xFFFF { 0 => 112, p => p as i64 }, rate: match request.data[1] >> 16 { 0 => 100, r => r as i64 } };
        // Текст копируется из страницы клиента, затем страница сразу отображается обратно.
        let mut text = [0u8; 4096]; let mut length = 0;
        if request.cap_received {
            if let Ok(page) = Mapping::new(RECEIVED_CAP) { length = len.min(page.len()).min(text.len()); text[..length].copy_from_slice(&page.as_slice()[..length]); }
            let _ = ipc::drop_cap(RECEIVED_CAP);
        }
        let result = match (op, core::str::from_utf8(&text[..length])) {
            (TTS_SAY, Ok(words)) if present && length > 0 => say(words, voice),
            (TTS_SAY, Ok(_)) if !present => Err(ERR_NOT_FOUND),
            _ => Err(ERR_INVALID),
        };
        match result { Ok(ms) => mind::println!("[TTS] SPOKE {} BYTES, {} MS", length, ms), Err(code) => mind::println!("[TTS] ERROR {:#x}", code) }
        if request.is_call { let _ = ipc::reply(&Message::new(result.unwrap_or_else(|code| code), 0)); }
    }
}
