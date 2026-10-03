// Host-side check of the tts synthesizer: text -> 16 kHz mono WAV (same modules as the ring 3 service).
// rustc --edition=2021 -O tests/tts_host.rs -o /tmp/tts_host && /tmp/tts_host "привет мир" out.wav
#![allow(dead_code)]
#[path = "../tts/src/dsp.rs"]
mod dsp;
#[path = "../tts/src/phonemes.rs"]
mod phonemes;
#[path = "../tts/src/synth.rs"]
mod synth;
#[path = "../tts/src/text.rs"]
mod text;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let mut units = [phonemes::Unit { ph: phonemes::Ph::Pause(0), soft: false, stress: false }; 2048];
    let count = text::parse(input, &mut units);
    if std::env::var("TTS_DEBUG").is_ok() { eprintln!("{:?}", &units[..count].iter().map(|u| (u.ph, u.soft)).collect::<Vec<_>>()); }
    let mut samples: Vec<i16> = Vec::new();
    synth::speak(&units[..count], synth::Voice::default(), &mut |chunk| samples.extend_from_slice(chunk));
    let mut wav = Vec::new();
    let data = (samples.len() * 2) as u32;
    wav.extend_from_slice(b"RIFF"); wav.extend_from_slice(&(36 + data).to_le_bytes()); wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); wav.extend_from_slice(&1u16.to_le_bytes()); wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&16_000u32.to_le_bytes()); wav.extend_from_slice(&32_000u32.to_le_bytes()); wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes()); wav.extend_from_slice(b"data"); wav.extend_from_slice(&data.to_le_bytes());
    for s in &samples { wav.extend_from_slice(&s.to_le_bytes()); }
    std::fs::write(output, wav).unwrap();
    let peak = samples.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
    println!("{} units, {} samples ({:.2} s), peak {}", count, samples.len(), samples.len() as f64 / 16000.0, peak);
}
