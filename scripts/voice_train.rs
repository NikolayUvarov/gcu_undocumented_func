//! Trains the voice recognizer's acoustic model (issue 078, docs/voice V1) and writes `voice/model.bin`.
//!
//!     rustc --edition=2021 -O scripts/voice_train.rs -o /tmp/voice_train && /tmp/voice_train voice/model.bin
//!
//! The training speech is our own synthesizer's (tts): the grammar's phrases (voice/commands.txt) and random strings of
//! Russian and English dictionary words, at random pitches and rates, in silence or white noise, with the phone of
//! every sample known from the synthesizer. Features are the target's own integer log-mel code
//! (libmind/src/voice/features.rs); the network is trained in f32 (Adam, cross-entropy), then quantized to int8 as
//! libmind/src/voice/model.rs reads it. The decoder's thresholds are calibrated on separate speech (other pitches, rates
//! and noise, and phrases outside the grammar) and stored in the model. Everything is seeded: the same sources give the
//! same model. Nothing here comes from outside the repository, so the model carries the synthesizer's licence terms
//! (its dictionaries: THIRD_PARTY.md).
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/voice/front.rs"]
mod front;
#[path = "../libmind/src/voice/features.rs"]
mod features;
#[path = "../libmind/src/voice/model.rs"]
mod model;
#[path = "../libmind/src/voice/grammar.rs"]
mod grammar;
#[path = "../libmind/src/voice/recognizer.rs"]
mod recognizer;
#[path = "../tts/src/dsp.rs"]
mod dsp;
#[path = "../phonetics/src/phonemes.rs"]
mod phonemes;
#[path = "../tts/src/synth.rs"]
mod synth;
#[path = "../phonetics/src/text.rs"]
mod text;
#[path = "voice_corpus.rs"]
mod corpus;

use corpus::{Rng, Take};
use features::{Features, BANDS};
use std::path::Path;

const CONTEXT: usize = 5;
const WINDOW: usize = 2 * CONTEXT + 1;
const INPUT: usize = BANDS * WINDOW;
const HIDDEN: [usize; 3] = [256, 256, 256];
/// Tenths of a dB per input unit; the network sees input units / 10.
const DIVISOR: i32 = 4;
const INPUT_SCALE: f32 = 0.1;
const THREADS: usize = 4;
const BATCH: usize = 512;
/// Seeds of the training and of the calibration speech.
const SEED: u64 = 0x5EED_0078;
const CALIBRATION_SEED: u64 = 0xCA11_0078;

/// One utterance: normalized log-mel features and the class of every frame.
struct Utterance { features: Vec<i16>, labels: Vec<u8> }

fn root() -> std::path::PathBuf { Path::new(file!()).parent().unwrap().parent().unwrap().to_path_buf() }

/// Russian and English words for random training strings.
fn words() -> (Vec<String>, Vec<String>) {
    let read = |name: &str| std::fs::read_to_string(root().join("phonetics/data").join(name)).unwrap();
    let ru = read("stress_ru.txt").lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(|l| l.to_lowercase()).collect();
    let en = read("lexicon_en.txt").lines().filter(|l| !l.starts_with('#') && !l.is_empty()).filter_map(|l| l.split_whitespace().next().map(str::to_string)).collect();
    (ru, en)
}

fn random_text(rng: &mut Rng, ru: &[String], en: &[String]) -> String {
    let list = if rng.uniform() < 0.5 { ru } else { en };
    (0..1 + rng.below(4)).map(|_| list[rng.below(list.len())].clone()).collect::<Vec<_>>().join(" ")
}

fn random_take(rng: &mut Rng, clean: f64) -> Take {
    Take { lead_ms: 30 + rng.below(370), trail_ms: 30 + rng.below(370), gain: rng.range(0.08, 1.0), snr: if rng.uniform() < clean { None } else { Some(rng.range(8.0, 35.0)) } }
}

/// Synthesizes, records and analyses `jobs` (text, pitch, rate, take, seed) on all cores.
fn utterances(jobs: Vec<(String, i64, i64, Take, u64)>) -> Vec<Utterance> {
    let features = Features::new();
    let chunks: Vec<Vec<(usize, (String, i64, i64, Take, u64))>> = {
        let mut chunks = vec![Vec::new(); THREADS];
        for (i, job) in jobs.into_iter().enumerate() { chunks[i % THREADS].push((i, job)); }
        chunks
    };
    let mut done: Vec<(usize, Utterance)> = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks.into_iter().map(|chunk| { let features = &features; scope.spawn(move || {
            chunk.into_iter().map(|(i, (text, pitch, rate, take, seed))| {
                let (speech, labels) = if text.is_empty() { (vec![0i16; 8000], vec![corpus::silence_class(); 8000]) } else { corpus::speak(&text, pitch, rate) };
                let (samples, labels) = corpus::record(&speech, &labels, take, &mut Rng(seed));
                let mut f = features.log_mel(&samples);
                features::normalize(&mut f);
                let frames = f.len() / BANDS;
                (i, Utterance { features: f, labels: corpus::frame_labels(&labels, frames) })
            }).collect::<Vec<_>>()
        }) }).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, u)| u).collect()
}

/// The network input of frame `t` (as the int8 model sees it, times INPUT_SCALE).
fn input(u: &Utterance, t: usize, out: &mut [f32]) {
    let frames = u.features.len() / BANDS;
    for w in 0..WINDOW {
        let f = (t + w).saturating_sub(CONTEXT).min(frames - 1);
        for b in 0..BANDS {
            let v = u.features[f * BANDS + b] as i32;
            let q = ((v + if v >= 0 { DIVISOR / 2 } else { -DIVISOR / 2 }) / DIVISOR).clamp(-127, 127);
            out[w * BANDS + b] = q as f32 * INPUT_SCALE;
        }
    }
}

#[derive(Clone)]
struct Layer { inputs: usize, outputs: usize, w: Vec<f32>, b: Vec<f32> }

#[derive(Clone)]
struct Net { layers: Vec<Layer> }

fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0f32; 8];
    let (ca, cb) = (a.chunks_exact(8), b.chunks_exact(8));
    let (ra, rb) = (ca.remainder(), cb.remainder());
    for (x, y) in ca.zip(cb) { for k in 0..8 { acc[k] += x[k] * y[k]; } }
    acc.iter().sum::<f32>() + ra.iter().zip(rb).map(|(x, y)| x * y).sum::<f32>()
}

fn axpy(alpha: f32, x: &[f32], y: &mut [f32]) { for (y, x) in y.iter_mut().zip(x) { *y += alpha * x; } }

impl Net {
    fn new(classes: usize, rng: &mut Rng) -> Self {
        let mut sizes = vec![INPUT];
        sizes.extend_from_slice(&HIDDEN);
        sizes.push(classes);
        let layers = sizes.windows(2).map(|s| {
            let scale = (2.0 / s[0] as f64).sqrt();
            Layer { inputs: s[0], outputs: s[1], w: (0..s[0] * s[1]).map(|_| (rng.gaussian() * scale) as f32).collect(), b: vec![0.0; s[1]] }
        }).collect();
        Self { layers }
    }

    /// Activations of every layer for one input (the last are logits).
    fn forward(&self, x: &[f32]) -> Vec<Vec<f32>> {
        let mut acts = vec![x.to_vec()];
        for (i, l) in self.layers.iter().enumerate() {
            let last = i + 1 == self.layers.len();
            let a = acts.last().unwrap();
            let z: Vec<f32> = (0..l.outputs).map(|o| { let v = l.b[o] + dot(&l.w[o * l.inputs..(o + 1) * l.inputs], a); if last { v } else { v.max(0.0) } }).collect();
            acts.push(z);
        }
        acts
    }

    /// Adds the gradient of the cross-entropy of one example to `grad`; returns the loss and whether it was right.
    fn backward(&self, x: &[f32], label: usize, grad: &mut Net) -> (f32, bool) {
        let acts = self.forward(x);
        let logits = acts.last().unwrap();
        let max = logits.iter().cloned().fold(f32::MIN, f32::max);
        let exps: Vec<f32> = logits.iter().map(|&z| (z - max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let right = logits.iter().enumerate().fold((0, f32::MIN), |b, (i, &z)| if z > b.1 { (i, z) } else { b }).0 == label;
        let mut delta: Vec<f32> = exps.iter().map(|e| e / sum).collect();
        let loss = -(delta[label].max(1e-12)).ln();
        delta[label] -= 1.0;
        for i in (0..self.layers.len()).rev() {
            let (l, g) = (&self.layers[i], &mut grad.layers[i]);
            let a = &acts[i];
            let mut previous = vec![0f32; l.inputs];
            for o in 0..l.outputs {
                let d = delta[o];
                if d == 0.0 { continue; }
                g.b[o] += d;
                axpy(d, a, &mut g.w[o * l.inputs..(o + 1) * l.inputs]);
                if i > 0 { axpy(d, &l.w[o * l.inputs..(o + 1) * l.inputs], &mut previous); }
            }
            if i > 0 { for (p, &a) in previous.iter_mut().zip(&acts[i]) { if a <= 0.0 { *p = 0.0; } } }
            delta = previous;
        }
        (loss, right)
    }

    fn zeroed(&self) -> Net { Net { layers: self.layers.iter().map(|l| Layer { inputs: l.inputs, outputs: l.outputs, w: vec![0.0; l.w.len()], b: vec![0.0; l.b.len()] }).collect() } }

    fn params(&mut self) -> Vec<&mut Vec<f32>> { self.layers.iter_mut().flat_map(|l| [&mut l.w, &mut l.b]).collect() }

    fn predict(&self, x: &[f32]) -> usize { self.forward(x).last().unwrap().iter().enumerate().fold((0, f32::MIN), |b, (i, &z)| if z > b.1 { (i, z) } else { b }).0 }
}

struct Adam { m: Net, v: Net, t: i32 }

impl Adam {
    fn step(&mut self, net: &mut Net, grad: &mut Net, lr: f32, scale: f32) {
        self.t += 1;
        let (b1, b2) = (0.9f32, 0.999f32);
        let (c1, c2) = (1.0 - b1.powi(self.t), 1.0 - b2.powi(self.t));
        let (mut p, mut g, mut m, mut v) = (net.params(), grad.params(), self.m.params(), self.v.params());
        for k in 0..p.len() {
            for i in 0..p[k].len() {
                let gi = g[k][i] * scale;
                m[k][i] = b1 * m[k][i] + (1.0 - b1) * gi;
                v[k][i] = b2 * v[k][i] + (1.0 - b2) * gi * gi;
                p[k][i] -= lr * (m[k][i] / c1) / ((v[k][i] / c2).sqrt() + 1e-8);
                g[k][i] = 0.0;
            }
        }
    }
}

fn train(net: &mut Net, data: &[Utterance], held: &[Utterance], epochs: usize, rng: &mut Rng) {
    let mut frames: Vec<(u32, u32)> = data.iter().enumerate().flat_map(|(u, x)| (0..x.labels.len() as u32).map(move |t| (u as u32, t))).collect();
    let mut adam = Adam { m: net.zeroed(), v: net.zeroed(), t: 0 };
    let mut grads: Vec<Net> = (0..THREADS).map(|_| net.zeroed()).collect();
    for epoch in 0..epochs {
        for i in (1..frames.len()).rev() { let j = rng.below(i + 1); frames.swap(i, j); }
        let lr = 1e-3 * if epoch >= 4 { 0.6f32.powi(epoch as i32 - 3) } else { 1.0 };
        let (mut loss, mut right, started) = (0f64, 0usize, std::time::Instant::now());
        for batch in frames.chunks(BATCH) {
            let shared = &*net;
            let results: Vec<(f32, usize)> = std::thread::scope(|scope| {
                let handles: Vec<_> = grads.iter_mut().zip(batch.chunks(batch.len().div_ceil(THREADS))).map(|(grad, part)| scope.spawn(move || {
                    let mut x = vec![0f32; INPUT];
                    let (mut loss, mut right) = (0f32, 0usize);
                    for &(u, t) in part {
                        input(&data[u as usize], t as usize, &mut x);
                        let (l, r) = shared.backward(&x, data[u as usize].labels[t as usize] as usize, grad);
                        loss += l; right += r as usize;
                    }
                    (loss, right)
                })).collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            for (l, r) in results { loss += l as f64; right += r; }
            let (first, rest) = grads.split_at_mut(1);
            for other in rest.iter_mut() {
                for (a, b) in first[0].params().into_iter().zip(other.params()) { for (x, y) in a.iter_mut().zip(b.iter_mut()) { *x += *y; *y = 0.0; } }
            }
            adam.step(net, &mut first[0], lr, 1.0 / batch.len() as f32);
        }
        let (mut held_right, mut held_total) = (0usize, 0usize);
        let mut x = vec![0f32; INPUT];
        for u in held { for t in 0..u.labels.len() { input(u, t, &mut x); held_right += (net.predict(&x) == u.labels[t] as usize) as usize; held_total += 1; } }
        eprintln!("epoch {}: loss {:.3}, train frames {:.1} %, held-out frames {:.1} % ({:.0} s)", epoch + 1, loss / frames.len() as f64,
                  100.0 * right as f64 / frames.len() as f64, 100.0 * held_right as f64 / held_total.max(1) as f64, started.elapsed().as_secs_f64());
    }
}

/// The int8 model file, with the decoder's thresholds.
fn quantize(net: &Net, classes: &[String], calibration: &[Utterance], accept: i32, margin: i32) -> Vec<u8> {
    // The hidden activations' scale: a high percentile of their values on calibration frames.
    let mut x = vec![0f32; INPUT];
    let mut values: Vec<Vec<f32>> = vec![Vec::new(); net.layers.len()];
    for u in calibration.iter().take(200) {
        for t in (0..u.labels.len()).step_by(3) {
            input(u, t, &mut x);
            let acts = net.forward(&x);
            for (i, a) in acts[1..].iter().enumerate() { values[i].extend(a.iter().filter(|&&v| v > 0.0)); }
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(model::MAGIC);
    for v in [model::VERSION, BANDS as u32, CONTEXT as u32, DIVISOR as u32, accept as u32, margin as u32, classes.len() as u32] { out.extend_from_slice(&v.to_le_bytes()); }
    for c in classes { out.push(c.len() as u8); out.extend_from_slice(c.as_bytes()); }
    out.extend_from_slice(&(net.layers.len() as u32).to_le_bytes());
    let mut s_in = INPUT_SCALE as f64;
    for (i, l) in net.layers.iter().enumerate() {
        let last = i + 1 == net.layers.len();
        let s_out = if last { 1.0 } else {
            let v = &mut values[i];
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            (v[((v.len() as f64 * 0.9999) as usize).min(v.len() - 1)] as f64 / 127.0).max(1e-6)
        };
        out.extend_from_slice(&(l.inputs as u32).to_le_bytes());
        out.extend_from_slice(&(l.outputs as u32).to_le_bytes());
        let scales: Vec<f64> = (0..l.outputs).map(|o| (l.w[o * l.inputs..(o + 1) * l.inputs].iter().fold(0f32, |m, w| m.max(w.abs())) as f64 / 127.0).max(1e-9)).collect();
        for o in 0..l.outputs { for &w in &l.w[o * l.inputs..(o + 1) * l.inputs] { out.push((w as f64 / scales[o]).round().clamp(-127.0, 127.0) as i8 as u8); } }
        for o in 0..l.outputs { out.extend_from_slice(&((l.b[o] as f64 / (scales[o] * s_in)).round() as i32).to_le_bytes()); }
        for o in 0..l.outputs {
            let m = if last { scales[o] * s_in * 256.0 * 65536.0 } else { scales[o] * s_in / s_out * (1u64 << 24) as f64 };
            out.extend_from_slice(&(m.round().min(i32::MAX as f64) as i32).to_le_bytes());
        }
        s_in = s_out;
    }
    let sum = model::checksum(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

fn pronounce(text: &str) -> Option<Vec<grammar::Token>> { corpus::pronounce(text) }

/// Phrases outside the grammar for calibration (tests/voice_host.rs has its own).
const CALIBRATION_OUTSIDE: [&str; 24] = [
    "открой окно", "покажи дорогу", "какая сегодня погода", "запусти ракету", "закрой дверь", "сколько стоит хлеб", "мне нужна помощь врача",
    "поставь чайник", "где мои ключи", "расскажи сказку", "включи свет", "давай поиграем",
    "open the door", "show me the way", "what is the weather", "start the engine", "close the window", "how much is it",
    "turn on the light", "where are my keys", "tell me a story", "play some music", "what is your name", "good morning",
];

/// The decoder's thresholds for `model` (a model file): the grammar spoken at other pitches and rates than the
/// training's, in 20 dB noise, and phrases outside it. The deficit threshold lies halfway between the worst phrase
/// recognized right and the best phrase outside the grammar.
fn calibrate(model: &[u8], grammar_text: &str, rng: &mut Rng) -> (i32, i32) {
    let recognizer = recognizer::Recognizer::new(model::Model::parse(model).unwrap(), grammar::Grammar::parse(grammar_text, &mut pronounce).unwrap()).unwrap();
    let phrases = &recognizer.grammar.phrases;
    let same = |a: usize, b: usize| phrases[a].intent == phrases[b].intent && phrases[a].slots == phrases[b].slots;
    let take = || Take { lead_ms: 150, trail_ms: 150, gain: 0.5, snr: Some(20.0) };
    let voices = [(105, 95), (140, 125)];
    let inside: Vec<(usize, u64, i64, i64)> = (0..phrases.len()).flat_map(|i| voices.map(|(p, r)| (i, rng.next(), p, r))).collect();
    let ins: Vec<(bool, i32, i32)> = inside.iter().map(|&(i, seed, pitch, rate)| {
        let (s, l) = corpus::speak(&phrases[i].text, pitch, rate);
        let r = recognizer.recognize(&corpus::record(&s, &l, take(), &mut Rng(seed)).0).unwrap();
        (same(r.decoded.phrase, i), r.decoded.deficit(), r.decoded.margin())
    }).collect();
    let outs: Vec<(i32, i32)> = CALIBRATION_OUTSIDE.iter().flat_map(|t| voices.map(|(pitch, rate)| {
        let (s, l) = corpus::speak(t, pitch, rate);
        let r = recognizer.recognize(&corpus::record(&s, &l, take(), &mut Rng(7)).0).unwrap();
        (r.decoded.deficit(), r.decoded.margin())
    })).collect();
    let mut right: Vec<i32> = ins.iter().filter(|x| x.0).map(|x| x.1).collect();
    right.sort();
    let mut outside: Vec<i32> = outs.iter().map(|x| x.0).collect();
    outside.sort();
    eprintln!("best phrase right: {}/{}; deficit of right phrases: median {}, 99th percentile {}, worst {}; of outside phrases: best {}, median {}",
              right.len(), ins.len(), right[right.len() / 2], right[right.len() * 99 / 100], right[right.len() - 1], outside[0], outside[outside.len() / 2]);
    // The 99th percentile rather than the worst: one odd phrase should not move the threshold toward the outside ones.
    let accept = (right[right.len() * 99 / 100] + outside[0]) / 2;
    let margin = 0;
    let ok = ins.iter().filter(|&&(r, d, m)| r && d <= accept && m >= margin).count();
    let rejected = outs.iter().filter(|&&(d, m)| d > accept || m < margin).count();
    eprintln!("thresholds: deficit <= {}, margin >= {} (1/256 nats per frame): {}/{} recognized, {}/{} outside rejected", accept, margin, ok, ins.len(), rejected, outs.len());
    (accept, margin)
}

/// Writes new thresholds into a model file (they are covered by its checksum).
fn set_thresholds(bytes: &mut Vec<u8>, accept: i32, margin: i32) {
    bytes[24..28].copy_from_slice(&(accept as u32).to_le_bytes());
    bytes[28..32].copy_from_slice(&(margin as u32).to_le_bytes());
    let end = bytes.len() - 4;
    let sum = model::checksum(&bytes[..end]);
    bytes[end..].copy_from_slice(&sum.to_le_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(at) = args.iter().position(|a| a == "--calibrate") {
        // Only new thresholds for an existing model.
        let path = &args[at + 1];
        let mut bytes = std::fs::read(path).unwrap();
        let grammar_text = std::fs::read_to_string(root().join("voice/commands.txt")).unwrap();
        let (accept, margin) = calibrate(&bytes, &grammar_text, &mut Rng(CALIBRATION_SEED));
        set_thresholds(&mut bytes, accept, margin);
        std::fs::write(path, &bytes).unwrap();
        eprintln!("wrote {}", path);
        return;
    }
    let quick = args.iter().any(|a| a == "--quick");
    let output = args.iter().skip(1).find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| root().join("voice/model.bin").to_string_lossy().into_owned());
    let classes = corpus::classes();
    let grammar_text = std::fs::read_to_string(root().join("voice/commands.txt")).unwrap();
    let grammar = grammar::Grammar::parse(&grammar_text, &mut pronounce).unwrap();
    let (ru, en) = words();
    eprintln!("{} classes, {} grammar phrases, {} + {} words", classes.len(), grammar.phrases.len(), ru.len(), en.len());
    let mut rng = Rng(SEED);
    // Training speech: every grammar phrase in several voices, random word strings, noise alone.
    let mut jobs = Vec::new();
    let voices = if quick { 2 } else { 6 };
    for p in &grammar.phrases { for _ in 0..voices { jobs.push((p.text.clone(), rng.range(82.0, 168.0) as i64, rng.range(75.0, 135.0) as i64, random_take(&mut rng, 0.15), rng.next())); } }
    for _ in 0..if quick { 600 } else { 4000 } { jobs.push((random_text(&mut rng, &ru, &en), rng.range(82.0, 168.0) as i64, rng.range(75.0, 135.0) as i64, random_take(&mut rng, 0.15), rng.next())); }
    for _ in 0..100 { jobs.push((String::new(), 100, 100, random_take(&mut rng, 0.3), rng.next())); }
    let started = std::time::Instant::now();
    let mut data = utterances(jobs);
    for i in (1..data.len()).rev() { let j = rng.below(i + 1); data.swap(i, j); }
    let held = data.split_off(data.len() - data.len() / 20);
    eprintln!("{} utterances, {} frames ({:.0} s)", data.len(), data.iter().map(|u| u.labels.len()).sum::<usize>(), started.elapsed().as_secs_f64());
    let mut net = Net::new(classes.len(), &mut rng);
    train(&mut net, &data, &held, if quick { 3 } else { 12 }, &mut rng);
    let probe = quantize(&net, &classes, &held, i32::MAX, i32::MIN);
    {
        let int8 = model::Model::parse(&probe).unwrap();
        let (mut float_right, mut int_right, mut total) = (0, 0, 0);
        let mut x = vec![0f32; INPUT];
        for u in held.iter().take(60) {
            let scores = int8.scores(&u.features);
            let k = classes.len();
            for t in 0..u.labels.len() {
                input(u, t, &mut x);
                float_right += (net.predict(&x) == u.labels[t] as usize) as usize;
                int_right += ((0..k).max_by_key(|&c| (scores[t * k + c], std::cmp::Reverse(c))).unwrap() == u.labels[t] as usize) as usize;
                total += 1;
            }
        }
        eprintln!("held-out frames: float {:.1} %, int8 {:.1} %", 100.0 * float_right as f64 / total as f64, 100.0 * int_right as f64 / total as f64);
    }
    let (accept, margin) = calibrate(&probe, &grammar_text, &mut Rng(CALIBRATION_SEED));
    let bytes = quantize(&net, &classes, &held, accept, margin);
    std::fs::write(&output, &bytes).unwrap();
    eprintln!("wrote {} ({} bytes)", output, bytes.len());
}
