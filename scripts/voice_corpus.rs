//! Speech for training and testing the voice recognizer (issue 078): phrases spoken by our synthesizer (tts) with the
//! phone class of every sample, silence and noise around them, and phrases spelled in the model's classes for the
//! grammar. Included by scripts/voice_train.rs and tests/voice_host.rs, which provide the modules `phonemes`, `text`,
//! `synth`, `dsp` and `grammar` at their roots.
use crate::grammar::Token;
use crate::phonemes::{Ph, Unit, PHONES};

pub const SILENCE: &str = "sil";
pub const RATE: usize = 16_000;

/// The model's classes: the phones, then silence.
pub fn classes() -> Vec<String> { PHONES.iter().map(|p| p.to_string()).chain(Some(SILENCE.to_string())).collect() }

pub fn silence_class() -> u8 { PHONES.len() as u8 }

pub fn units(text: &str) -> Vec<Unit> {
    let mut units = vec![Unit { ph: Ph::Pause(0), soft: false, stress: false }; 4096];
    let count = crate::text::parse(text, &mut units);
    units.truncate(count);
    units
}

fn class_of(unit: &Unit) -> u8 {
    unit.ph.name().map_or(silence_class(), |name| PHONES.iter().position(|p| *p == name).unwrap() as u8)
}

/// A phrase spelled in the classes, with a possible pause between words (what `Grammar::parse` wants).
pub fn pronounce(text: &str) -> Option<Vec<Token>> {
    let mut out = Vec::new();
    for unit in units(text) {
        match unit.ph {
            Ph::Pause(0) => {}
            Ph::Pause(_) | Ph::End(_) => if !out.is_empty() && out.last() != Some(&Token::Pause) { out.push(Token::Pause) },
            _ => out.push(Token::Phone(class_of(&unit) as u16)),
        }
    }
    while out.last() == Some(&Token::Pause) { out.pop(); }
    Some(out)
}

/// `text` spoken at `pitch` Hz and `rate` per cent, with the class of every sample.
pub fn speak(text: &str, pitch: i64, rate: i64) -> (Vec<i16>, Vec<u8>) {
    let units = units(text);
    let (mut samples, mut labels) = (Vec::new(), Vec::new());
    crate::synth::speak_labeled(&units, crate::synth::Voice { pitch, rate }, &mut |chunk, index| {
        samples.extend_from_slice(chunk);
        labels.extend(std::iter::repeat(class_of(&units[index])).take(chunk.len()));
    });
    (samples, labels)
}

/// A deterministic random generator (xorshift64*).
#[derive(Clone)]
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    pub fn uniform(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 }
    pub fn range(&mut self, low: f64, high: f64) -> f64 { low + (high - low) * self.uniform() }
    pub fn below(&mut self, n: usize) -> usize { (self.next() % n.max(1) as u64) as usize }
    pub fn gaussian(&mut self) -> f64 { let (u, v) = (self.uniform().max(1e-12), self.uniform()); (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos() }
}

/// How an utterance is recorded: silence before and after it (ms), its gain, and white noise `snr` dB below the
/// speech (None: none).
#[derive(Clone, Copy, Debug)]
pub struct Take { pub lead_ms: usize, pub trail_ms: usize, pub gain: f64, pub snr: Option<f64> }

/// The speech placed in silence and noise as `take` says; labels follow (silence around it).
pub fn record(speech: &[i16], labels: &[u8], take: Take, rng: &mut Rng) -> (Vec<i16>, Vec<u8>) {
    let (lead, trail) = (take.lead_ms * RATE / 1000, take.trail_ms * RATE / 1000);
    let mut signal: Vec<f64> = vec![0.0; lead];
    signal.extend(speech.iter().map(|&s| s as f64 * take.gain));
    signal.extend(std::iter::repeat(0.0).take(trail));
    let mut out_labels = vec![silence_class(); lead];
    out_labels.extend_from_slice(labels);
    out_labels.extend(std::iter::repeat(silence_class()).take(trail));
    if let Some(snr) = take.snr {
        let voiced: Vec<f64> = signal.iter().zip(&out_labels).filter(|(_, &l)| l != silence_class()).map(|(s, _)| s * s).collect();
        let power = voiced.iter().sum::<f64>() / voiced.len().max(1) as f64;
        let sigma = (power.max(1.0) / 10f64.powf(snr / 10.0)).sqrt();
        for s in signal.iter_mut() { *s += sigma * rng.gaussian(); }
    }
    (signal.iter().map(|s| s.round().clamp(-32768.0, 32767.0) as i16).collect(), out_labels)
}

/// The label of every 10 ms frame (the sample at the frame's centre).
pub fn frame_labels(labels: &[u8], frames: usize) -> Vec<u8> {
    (0..frames).map(|f| labels[(f * 160 + 200).min(labels.len() - 1)]).collect()
}
