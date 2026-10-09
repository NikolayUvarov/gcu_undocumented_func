// Host-side check of the tts synthesizer: text -> 16 kHz mono WAV (same modules as the ring 3 service).
// rustc --edition=2021 -O tests/tts_host.rs -o /tmp/tts_host && /tmp/tts_host "привет мир" out.wav
#![allow(dead_code)]
#[path = "../tts/src/dsp.rs"]
mod dsp;
#[path = "../phonetics/src/phonemes.rs"]
mod phonemes;
#[path = "../tts/src/synth.rs"]
mod synth;
#[path = "../phonetics/src/text.rs"]
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

/// Every pause between words and the end of every phrase is digital silence from 50 ms on: rounding the filters'
/// feedback down kept a quiet tone of about −55 dBFS going until the next sound (issue 087).
#[test]
fn pauses_and_phrase_ends_fall_silent() {
    use phonemes::Ph;
    let phrases = ["hello world", "the quick brown fox jumps over the lazy dog.", "what time is it?", "открой файлы",
                   "привет, мир. как дела?", "сегодня хорошая погода, не правда ли"];
    for voice in [synth::Voice::default(), synth::Voice { pitch: 85, rate: 80 }, synth::Voice { pitch: 160, rate: 130 }] {
        for phrase in phrases {
            let mut units = [phonemes::Unit { ph: Ph::Pause(0), soft: false, stress: false }; 2048];
            let count = text::parse(phrase, &mut units);
            let mut labeled: Vec<(i16, usize)> = Vec::new();
            synth::speak_labeled(&units[..count], voice, &mut |chunk, index| labeled.extend(chunk.iter().map(|&s| (s, index))));
            let mut checked = 0;
            for (index, unit) in units[..count].iter().enumerate().filter(|(_, u)| matches!(u.ph, Ph::Pause(_) | Ph::End(_))) {
                let samples: Vec<i16> = labeled.iter().filter(|(_, i)| *i == index).map(|(s, _)| *s).collect();
                if samples.len() <= 50 * 16 { continue; }
                let late = samples[50 * 16..].iter().position(|&s| s != 0).map(|at| (50 * 16 + at) / 16);
                assert_eq!(late, None, "{:?} (pitch {}, rate {}): sound {:?} ms into {:?}", phrase, voice.pitch, voice.rate, late, unit.ph);
                checked += 1;
            }
            assert!(checked >= 2, "{:?}: a pause between words and the end", phrase);
        }
    }
}

/// No sound starts with a step (252-APP-0041): amplitudes changed once a frame (5 ms), so a burst, a fricative or a
/// voice after silence rose from 0 to thousands within a sample, a click on small speakers (223 such onsets in the 24
/// Russian sentences of scripts/voice_tts/sentences.tsv). They now move over 2 ms.
#[test]
fn sounds_start_without_a_step() {
    let phrases = ["кот", "сок", "привет, мир. как дела?", "сегодня хорошая погода, не правда ли", "Говорит разум корабля. Все системы работают нормально, курс проложен.",
                   "Запускаю файловый менеджер. Свободно восемь гигабайт памяти из шестнадцати.", "the quick brown fox jumps over the lazy dog.", "open the files"];
    for phrase in phrases {
        let mut units = [phonemes::Unit { ph: phonemes::Ph::Pause(0), soft: false, stress: false }; 2048];
        let count = text::parse(phrase, &mut units);
        let mut samples: Vec<i16> = Vec::new();
        synth::speak(&units[..count], synth::Voice::default(), &mut |chunk| samples.extend_from_slice(chunk));
        // After 10 ms of silence, the first three samples stay small.
        for n in 160..samples.len() - 3 {
            if samples[n] != 0 && samples[n - 160..n].iter().all(|&s| s == 0) {
                let peak = samples[n..n + 3].iter().map(|&s| (s as i32).abs()).max().unwrap();
                assert!(peak <= 2000, "{:?}: a step to {} at {} ms", phrase, peak, n / 16);
            }
        }
    }
}
