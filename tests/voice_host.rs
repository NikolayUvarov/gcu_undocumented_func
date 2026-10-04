//! Host tests of the voice front end (libmind/src/voice/front.rs): WAV parsing, conversion to 16 kHz mono and speech
//! detection on speech from our own synthesizer (tts/src), in silence and in white noise.
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/voice/front.rs"]
mod front;
#[path = "../tts/src/dsp.rs"]
mod dsp;
#[path = "../tts/src/phonemes.rs"]
mod phonemes;
#[path = "../tts/src/synth.rs"]
mod synth;
#[path = "../tts/src/text.rs"]
mod text;

use front::{Detector, Resampler, Source, Stream, Utterance, Wav, WavError};

const MS: usize = 16; // samples per millisecond at 16 kHz

fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF"); out.extend_from_slice(&(36 + data).to_le_bytes()); out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes()); out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes()); out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes()); out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data"); out.extend_from_slice(&data.to_le_bytes());
    for s in samples { out.extend_from_slice(&s.to_le_bytes()); }
    out
}

fn tone(rate: u32, channels: usize, hz: f64, amplitude: f64, ms: usize) -> Vec<i16> {
    let frames = rate as usize * ms / 1000;
    (0..frames).flat_map(|i| { let v = (amplitude * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin()).round() as i16; std::iter::repeat(v).take(channels) }).collect()
}

fn rms(samples: &[i16]) -> f64 { (samples.iter().map(|&s| s as f64 * s as f64).sum::<f64>() / samples.len().max(1) as f64).sqrt() }

/// 16 kHz mono of `samples`, fed in uneven chunks.
fn convert(rate: u32, channels: usize, samples: &[i16]) -> Vec<i16> {
    let mut resampler = Resampler::new(rate, channels);
    let mut out = Vec::new();
    for chunk in samples.chunks(channels * 997) { resampler.process(chunk, &mut out); }
    out
}

/// Gain in dB of a tone through the conversion (the first 50 ms of filter start-up skipped).
fn gain(rate: u32, channels: usize, hz: f64) -> f64 {
    let input = tone(rate, channels, hz, 10_000.0, 500);
    let out = convert(rate, channels, &input);
    let expected = rate as usize / 2 * 16_000 / rate as usize; // 500 ms at 16 kHz
    assert!((out.len() as i64 - expected as i64).abs() <= 1, "{} Hz: {} samples instead of {}", rate, out.len(), expected);
    20.0 * (rms(&out[50 * MS..]) / (10_000.0 / 2f64.sqrt())).log10()
}

#[test]
fn resampler_passes_speech_and_stops_aliases() {
    for (rate, channels) in [(48_000, 2), (48_000, 1), (44_100, 2), (32_000, 1), (22_050, 1), (8_000, 1)] {
        let g = gain(rate, channels, 1_000.0);
        assert!(g.abs() < 0.2, "{} Hz x{}: 1 kHz gain {:.2} dB", rate, channels, g);
    }
    for rate in [48_000, 44_100, 32_000] {
        let g = gain(rate, 2, 10_000.0);
        assert!(g < -40.0, "{} Hz: 10 kHz must not fold back into the band ({:.1} dB)", rate, g);
        let g = gain(rate, 2, 6_000.0);
        assert!(g.abs() < 0.5, "{} Hz: 6 kHz is still speech ({:.2} dB)", rate, g);
    }
    // Upsampling 8 kHz: a 3 kHz tone keeps its level, its 5 kHz image is gone.
    let out = convert(8_000, 1, &tone(8_000, 1, 3_000.0, 10_000.0, 500));
    let power = |hz: f64| { let (mut re, mut im) = (0.0, 0.0); for (i, &s) in out[50 * MS..].iter().enumerate() { let a = 2.0 * std::f64::consts::PI * hz * i as f64 / 16_000.0; re += s as f64 * a.cos(); im -= s as f64 * a.sin(); } (re * re + im * im).sqrt() };
    assert!(power(5_000.0) < power(3_000.0) / 100.0, "image at 5 kHz");
    // 16 kHz passes through untouched; stereo is averaged.
    let input: Vec<i16> = (0..1600).map(|i| ((i * 37) % 2001) as i16 - 1000).collect();
    assert!(Resampler::new(16_000, 1).passthrough());
    assert_eq!(convert(16_000, 1, &input), input);
    let stereo: Vec<i16> = input.iter().flat_map(|&s| [s, s.wrapping_neg()]).collect();
    assert!(convert(16_000, 2, &stereo).iter().all(|&s| s == 0), "L and -R cancel");
    assert!(convert(48_000, 2, &tone(48_000, 2, 1_000.0, 10_000.0, 100)).len() == 1600);
}

#[test]
fn wav_files() {
    let samples = tone(48_000, 2, 440.0, 8_000.0, 100);
    let mut file = Wav::parse(wav(48_000, 2, &samples)).unwrap();
    assert_eq!((file.rate(), file.channels(), file.frames(), file.duration_ms()), (48_000, 2, 4_800, 100));
    let mut read = vec![0i16; 9_601];
    let mut got = Vec::new();
    while !file.finished() { let n = file.read(&mut read[..1001]).unwrap(); assert!(n % 2 == 0, "whole frames"); got.extend_from_slice(&read[..n]); }
    assert_eq!(got, samples);
    assert_eq!(file.read(&mut read).unwrap(), 0);
    file.rewind();
    assert_eq!(file.read(&mut read).unwrap(), 9_600);
    // Other chunks are skipped (with the pad byte of an odd size); a streamed data size is cut to the file.
    let mut bytes = wav(16_000, 1, &[1, 2, 3]);
    let list = [b"LIST".as_slice(), &3u32.to_le_bytes(), b"abc\0"].concat();
    bytes.splice(36..36, list);
    bytes.extend_from_slice(&[0x55]); // half a sample
    let len = bytes.len();
    bytes[len - 11..len - 7].copy_from_slice(&u32::MAX.to_le_bytes()); // the data size
    let mut file = Wav::parse(bytes).unwrap();
    assert_eq!(file.frames(), 3);
    assert_eq!(file.read(&mut read).unwrap(), 3);
    assert_eq!(&read[..3], [1, 2, 3]);
    // WAVE_FORMAT_EXTENSIBLE with PCM.
    let mut bytes = wav(22_050, 1, &[7]);
    bytes[16..20].copy_from_slice(&40u32.to_le_bytes());
    bytes[20..22].copy_from_slice(&0xFFFEu16.to_le_bytes());
    let mut extension = vec![22u8, 0, 16, 0, 0, 0, 0, 0, 1, 0];
    extension.extend_from_slice(&[0; 14]);
    bytes.splice(36..36, extension);
    assert_eq!(Wav::parse(bytes).unwrap().rate(), 22_050);
    // Refused.
    assert_eq!(Wav::parse(b"RIFX....WAVE".to_vec()).err(), Some(WavError::NotWav));
    assert_eq!(Wav::parse(Vec::new()).err(), Some(WavError::NotWav));
    let mut eight_bit = wav(8_000, 1, &[0]);
    eight_bit[34] = 8;
    assert_eq!(Wav::parse(eight_bit).err(), Some(WavError::NotPcm16));
    let mut float = wav(8_000, 1, &[0]);
    float[20] = 3;
    assert_eq!(Wav::parse(float).err(), Some(WavError::NotPcm16));
    let mut no_data = wav(8_000, 1, &[]);
    no_data.truncate(36);
    assert_eq!(Wav::parse(no_data).err(), Some(WavError::NoData));
    let mut nine_channels = wav(8_000, 1, &[0]);
    nine_channels[22] = 9;
    assert_eq!(Wav::parse(nine_channels).err(), Some(WavError::BadFormat));
}

#[test]
fn stream_converts_a_source() {
    let samples = tone(48_000, 2, 1_000.0, 10_000.0, 300);
    let mut stream = Stream::new(Wav::parse(wav(48_000, 2, &samples)).unwrap());
    let mut out = Vec::new();
    while !stream.finished() { stream.read(&mut out).unwrap(); }
    assert_eq!(out.len(), 300 * MS);
    assert_eq!(stream.read(&mut out).unwrap(), 0);
    assert_eq!(out, convert(48_000, 2, &samples));
}

#[test]
fn decibels() {
    assert_eq!(front::decibels(0), 0);
    assert_eq!(front::decibels(1), 0);
    for energy in [2u64, 10, 1_000, 123_456, 1 << 30, 32_767 * 32_767, u32::MAX as u64 * 4] {
        let exact = 100.0 * (energy as f64).log10();
        assert!((front::decibels(energy) as f64 - exact).abs() <= 1.0, "{}: {} vs {:.1}", energy, front::decibels(energy), exact);
    }
    assert_eq!(front::dbfs(903), 0);
    assert_eq!(front::dbfs(903 - 64), -6);
    assert_eq!(front::dbfs(903 - 65), -7);
    assert_eq!(front::dbfs(0), -90);
}

// ---- Speech ----

fn speak(text: &str) -> Vec<i16> {
    let mut units = [phonemes::Unit { ph: phonemes::Ph::Pause(0), soft: false, stress: false }; 2048];
    let count = text::parse(text, &mut units);
    let mut samples = Vec::new();
    synth::speak(&units[..count], synth::Voice::default(), &mut |chunk| samples.extend_from_slice(chunk));
    samples
}

/// Where the sound of a phrase is: the first and last 5 ms block within 35 dB of its loudest block.
fn bounds(samples: &[i16]) -> (usize, usize) {
    let blocks: Vec<f64> = samples.chunks(5 * MS).map(rms).collect();
    let loudest = blocks.iter().cloned().fold(0.0, f64::max);
    let audible = |b: &f64| *b >= loudest * 10f64.powf(-35.0 / 20.0);
    let first = blocks.iter().position(audible).unwrap();
    let last = blocks.iter().rposition(audible).unwrap();
    (first * 5 * MS, ((last + 1) * 5 * MS).min(samples.len()))
}

const PHRASES: [&str; 4] = ["открой файлы", "который час?", "покажи процессы", "hello world"];

/// The phrases with silence between them: the signal and each phrase's bounds in it.
fn compose(phrases: &[&str], gap_ms: usize) -> (Vec<i16>, Vec<(usize, usize)>) {
    let mut signal = vec![0i16; gap_ms / 2 * MS];
    let mut spans = Vec::new();
    for phrase in phrases {
        let speech = speak(phrase);
        let (from, to) = bounds(&speech);
        spans.push((signal.len() + from, signal.len() + to));
        signal.extend_from_slice(&speech);
        signal.extend(std::iter::repeat(0).take(gap_ms * MS));
    }
    (signal, spans)
}

/// A deterministic Gaussian noise generator.
struct Noise(u64);
impl Noise {
    fn uniform(&mut self) -> f64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 11) as f64 / (1u64 << 53) as f64 }
    fn gaussian(&mut self) -> f64 { let (u, v) = (self.uniform().max(1e-12), self.uniform()); (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos() }
}

/// Adds white noise `snr` dB below the mean power of the speech in `spans`.
fn add_noise(signal: &mut [i16], spans: &[(usize, usize)], snr: f64, seed: u64) {
    let (energy, count) = spans.iter().fold((0.0, 0usize), |(e, n), &(a, b)| (e + signal[a..b].iter().map(|&s| s as f64 * s as f64).sum::<f64>(), n + b - a));
    let sigma = (energy / count as f64 / 10f64.powf(snr / 10.0)).sqrt();
    let mut noise = Noise(seed);
    for s in signal.iter_mut() { *s = (*s as f64 + sigma * noise.gaussian()).round().clamp(-32768.0, 32767.0) as i16; }
}

fn detect(signal: &[i16]) -> Vec<Utterance> {
    let mut detector = Detector::new();
    let mut found = Vec::new();
    for chunk in signal.chunks(1_234) { detector.push(chunk, &mut |u| found.push(u)); }
    detector.finish(&mut |u| found.push(u));
    found
}

fn check(signal: &[i16], spans: &[(usize, usize)], what: &str) {
    let found = detect(signal);
    let report: Vec<(u64, u64)> = found.iter().map(|u| (u.start_ms, u.start_ms + u.length_ms())).collect();
    let expected: Vec<(usize, usize)> = spans.iter().map(|&(a, b)| (a / MS, b / MS)).collect();
    assert_eq!(found.len(), spans.len(), "{}: found {:?}, expected {:?}", what, report, expected);
    for (u, &(from, to)) in found.iter().zip(spans) {
        let (start, end) = (u.start_ms as i64, (u.start_ms + u.length_ms()) as i64);
        assert!((start - (from / MS) as i64).abs() <= 50 && (end - (to / MS) as i64).abs() <= 50, "{}: found {:?}, expected {:?}", what, report, expected);
        let at = u.start_ms as usize * MS;
        assert_eq!(&u.samples[..], &signal[at..at + u.samples.len()], "{}: the utterance's samples are the stream's", what);
        assert!((-40..=-3).contains(&u.level), "{}: level {} dBFS", what, u.level);
    }
}

#[test]
fn finds_every_phrase_in_silence() {
    let (signal, spans) = compose(&PHRASES, 1_000);
    check(&signal, &spans, "silence");
}

#[test]
fn finds_every_phrase_in_noise() {
    for seed in [1, 2, 3] {
        let (mut signal, spans) = compose(&PHRASES, 1_000);
        add_noise(&mut signal, &spans, 20.0, seed);
        check(&signal, &spans, &format!("20 dB SNR, seed {}", seed));
    }
}

#[test]
fn nothing_in_noise_or_silence() {
    let mut noise = vec![0i16; 5_000 * MS];
    assert!(detect(&noise).is_empty(), "digital silence");
    add_noise(&mut noise, &[(0, 1)], 0.0, 7); // noise of RMS 1
    let mut louder = vec![0i16; 5_000 * MS];
    louder[0] = 3_000;
    add_noise(&mut louder, &[(0, 1)], 0.0, 9); // RMS 3000: loud noise
    for (signal, what) in [(&noise, "quiet noise"), (&louder, "loud noise")] {
        let found = detect(signal);
        assert!(found.is_empty(), "{}: {:?}", what, found.iter().map(|u| (u.start_ms, u.length_ms())).collect::<Vec<_>>());
    }
    // A click (100 ms) is too short to be speech.
    let mut click = vec![0i16; 2_000 * MS];
    for (i, s) in click[500 * MS..600 * MS].iter_mut().enumerate() { *s = if i % 20 < 10 { 8_000 } else { -8_000 }; }
    assert!(detect(&click).is_empty(), "a click");
}

#[test]
fn long_speech_is_cut() {
    // 10 s of a buzzing vowel without pauses: an 8 s utterance, then the rest.
    let mut signal = vec![0i16; 500 * MS];
    signal.extend((0..10_000 * MS).map(|i| { let t = i as f64 / 16_000.0; (6_000.0 * (2.0 * std::f64::consts::PI * 140.0 * t).sin() + 3_000.0 * (2.0 * std::f64::consts::PI * 700.0 * t).sin()) as i16 }));
    signal.extend(std::iter::repeat(0).take(1_000 * MS));
    let found = detect(&signal);
    let report: Vec<(u64, u64)> = found.iter().map(|u| (u.start_ms, u.length_ms())).collect();
    assert_eq!(found.len(), 2, "{:?}", report);
    assert!(found[0].length_ms() == 8_000 && (found[0].start_ms as i64 - 500).abs() <= 20, "{:?}", report);
    assert!((found[1].start_ms as i64 - 8_500).abs() <= 20 && (found[1].length_ms() as i64 - 2_000).abs() <= 40, "{:?}", report);
}

#[test]
fn level_follows_loudness() {
    let (signal, _) = compose(&PHRASES[..1], 1_000);
    let quieter: Vec<i16> = signal.iter().map(|&s| s / 4).collect();
    let (loud, quiet) = (detect(&signal), detect(&quieter));
    assert_eq!((loud.len(), quiet.len()), (1, 1));
    assert!((loud[0].level - quiet[0].level - 12).abs() <= 1, "{} vs {} dBFS", loud[0].level, quiet[0].level);
}
