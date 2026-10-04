//! The acoustic model of the voice recognizer (docs/voice, V1): a small network with int8 weights that turns log-mel
//! features into per-frame scores of the phones (`phonetics::phonemes::PHONES`) and silence. The first layer sees 11
//! frames around the current one (a convolution over time), the others are dense; ReLU between them. Trained on the
//! host by scripts/voice_train.rs, which writes this format; host tests include this file.
//!
//! File (little endian): `MINDVOX1`, version u32 (1), bands u32, context u32 (frames on each side), input divisor u32
//! (tenths of a dB per input unit), the decoder's thresholds calibrated with the model — the largest mean deficit per
//! frame a phrase may have and the smallest margin per frame over a phrase meaning something else (u32 each, 1/256
//! nats) — classes u32, then per class a length byte and its name; layers u32, then per layer
//! inputs u32, outputs u32, then `outputs * inputs` int8 weights (row by row), `outputs` i32 biases and `outputs` i32
//! multipliers; finally an FNV-1a checksum u32 of everything before it. A hidden layer's output is
//! `clamp((acc + bias) * multiplier >> 24, 0, 127)`; the last layer's is `(acc + bias) * multiplier >> 16`, a score
//! in 1/256 nats.
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub const MAGIC: &[u8; 8] = b"MINDVOX1";
pub const VERSION: u32 = 1;
/// Scores are in 1/256 nats.
pub const SCORE_ONE: i32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError { Magic, Version, Truncated, Checksum, Shape }

pub struct Layer { pub inputs: usize, pub outputs: usize, pub weights: Vec<i8>, pub bias: Vec<i32>, pub multiplier: Vec<i32> }

pub struct Model { pub bands: usize, pub context: usize, pub divisor: i32, pub accept: i32, pub margin: i32, pub classes: Vec<String>, pub layers: Vec<Layer> }

/// FNV-1a over `bytes`.
pub fn checksum(bytes: &[u8]) -> u32 { bytes.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193)) }

struct Reader<'a> { bytes: &'a [u8], at: usize }
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ModelError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or(ModelError::Truncated)?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn u32(&mut self) -> Result<u32, ModelError> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn i32s(&mut self, n: usize) -> Result<Vec<i32>, ModelError> { Ok(self.take(n * 4)?.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect()) }
}

impl Model {
    pub fn parse(bytes: &[u8]) -> Result<Self, ModelError> {
        if bytes.len() < 12 || &bytes[..8] != MAGIC { return Err(ModelError::Magic); }
        let body = &bytes[..bytes.len() - 4];
        if checksum(body) != u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap()) { return Err(ModelError::Checksum); }
        let mut r = Reader { bytes: body, at: 8 };
        if r.u32()? != VERSION { return Err(ModelError::Version); }
        let (bands, context, divisor) = (r.u32()? as usize, r.u32()? as usize, r.u32()? as i32);
        let (accept, margin) = (r.u32()? as i32, r.u32()? as i32);
        let count = r.u32()? as usize;
        if count == 0 || count > 256 || bands == 0 || bands > 128 || context > 32 || divisor <= 0 { return Err(ModelError::Shape); }
        let mut classes = Vec::with_capacity(count);
        for _ in 0..count {
            let len = r.take(1)?[0] as usize;
            classes.push(String::from(core::str::from_utf8(r.take(len)?).map_err(|_| ModelError::Shape)?));
        }
        let mut layers: Vec<Layer> = Vec::new();
        let mut width = bands * (2 * context + 1);
        for _ in 0..r.u32()? {
            let (inputs, outputs) = (r.u32()? as usize, r.u32()? as usize);
            if inputs != width || outputs == 0 || outputs > 4096 { return Err(ModelError::Shape); }
            let weights = r.take(inputs * outputs)?.iter().map(|&b| b as i8).collect();
            let (bias, multiplier) = (r.i32s(outputs)?, r.i32s(outputs)?);
            layers.push(Layer { inputs, outputs, weights, bias, multiplier });
            width = outputs;
        }
        if layers.is_empty() || width != count || r.at != body.len() { return Err(ModelError::Shape); }
        Ok(Self { bands, context, divisor, accept, margin, classes, layers })
    }

    /// The class index of `name`.
    pub fn class(&self, name: &str) -> Option<usize> { self.classes.iter().position(|c| c == name) }

    /// Scores of every class for every frame of normalized log-mel `features` (frame after frame, `bands` per frame):
    /// frame after frame, `classes` per frame, each the class's output minus the frame's best (so the best is 0).
    pub fn scores(&self, features: &[i16]) -> Vec<i32> {
        let frames = features.len() / self.bands;
        let window = 2 * self.context + 1;
        let widest = self.layers.iter().map(|l| l.outputs).max().unwrap_or(0).max(self.bands * window);
        let (mut input, mut output) = (vec![0i8; widest], vec![0i32; widest]);
        let mut out = Vec::with_capacity(frames * self.classes.len());
        for t in 0..frames {
            // The frames around t (the edges repeat), quantized.
            for w in 0..window {
                let f = (t + w).saturating_sub(self.context).min(frames - 1);
                for b in 0..self.bands {
                    let v = features[f * self.bands + b] as i32;
                    input[w * self.bands + b] = ((v + if v >= 0 { self.divisor / 2 } else { -self.divisor / 2 }) / self.divisor).clamp(-127, 127) as i8;
                }
            }
            for (index, layer) in self.layers.iter().enumerate() {
                let last = index + 1 == self.layers.len();
                for o in 0..layer.outputs {
                    let row = &layer.weights[o * layer.inputs..(o + 1) * layer.inputs];
                    let acc: i32 = row.iter().zip(&input[..layer.inputs]).map(|(&w, &x)| w as i32 * x as i32).sum();
                    let value = (acc + layer.bias[o]) as i64 * layer.multiplier[o] as i64;
                    output[o] = if last { (value >> 16) as i32 } else { (value >> 24).clamp(0, 127) as i32 };
                }
                if !last { for o in 0..layer.outputs { input[o] = output[o] as i8; } }
            }
            let classes = self.classes.len();
            let best = output[..classes].iter().copied().max().unwrap_or(0);
            out.extend(output[..classes].iter().map(|&s| s - best));
        }
        out
    }
}
