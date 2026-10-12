//! Dictation (250): speech to text with a transducer in a network file (`mind::nn`): Kaldi's features of the whole
//! utterance, the encoder over them, and greedy search over the decoder and the joiner, one symbol at most per frame,
//! as sherpa-onnx searches. The file holds the graphs `encoder`, `decoder` and `joiner` and the BPE tokens; token 0 is
//! the blank. The text is data: nothing here runs it (MC-11.5). Host tests include this file.
use super::fbank::{Fbank, BANDS};
use crate::nn::{op_error, Data, Model, Result, Tensor};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Tokens the decoder sees: the last two (icefall's stateless decoder).
const CONTEXT: usize = 2;

/// A recognizer over a network file.
pub struct Dictation<'f> { model: Model<'f>, fbank: Fbank }

impl<'f> Dictation<'f> {
    /// Refuses a file without the three graphs or without tokens.
    pub fn new(model: Model<'f>) -> Result<Self> {
        for graph in ["encoder", "decoder", "joiner"] { if !model.has(graph) { return Err(op_error(alloc::format!("no graph {}", graph))); } }
        if model.tokens.is_empty() { return Err(op_error("no tokens")); }
        Ok(Self { model, fbank: Fbank::new() })
    }

    pub fn model(&self) -> &Model<'f> { &self.model }

    /// The text of 16 kHz mono samples.
    pub fn text(&self, samples: &[i16]) -> Result<String> { self.text_of_features(&self.fbank.compute_i16(samples)) }

    /// The text of features, `BANDS` per frame.
    pub fn text_of_features(&self, features: &[f32]) -> Result<String> {
        let frames = features.len() / BANDS;
        if frames == 0 { return Ok(String::new()); }
        let out = self.model.run("encoder", vec![Tensor::f32(vec![1, frames, BANDS], features[..frames * BANDS].to_vec()), Tensor::i64(vec![1], vec![frames as i64])])?;
        let encoded = out.first().ok_or_else(|| op_error("the encoder made nothing"))?;
        let (Data::F32(values), &[1, t, dim]) = (&encoded.data, &encoded.shape[..]) else { return Err(op_error("the encoder's output is not [1, T, D] f32")) };
        self.search(values, t, dim)
    }

    /// Greedy search over `t` encoder frames of `dim` values each.
    pub fn search(&self, encoded: &[f32], t: usize, dim: usize) -> Result<String> {
        let mut context = [0i64; CONTEXT];
        let decode = |context: &[i64; CONTEXT]| -> Result<Tensor> {
            self.model.run("decoder", vec![Tensor::i64(vec![1, CONTEXT], context.to_vec())])?.into_iter().next().ok_or_else(|| op_error("the decoder made nothing"))
        };
        let mut decoded = decode(&context)?;
        let mut found: Vec<usize> = Vec::new();
        for frame in encoded.chunks_exact(dim).take(t) {
            let logits = self.model.run("joiner", vec![Tensor::f32(vec![1, dim], frame.to_vec()), decoded.clone()])?;
            let Some(Tensor { data: Data::F32(l), .. }) = logits.first() else { return Err(op_error("the joiner's output is not f32")) };
            let best = l.iter().enumerate().fold((0, f32::NEG_INFINITY), |b, (k, &v)| if v > b.1 { (k, v) } else { b }).0;
            if best != 0 {
                found.push(best);
                context.rotate_left(1);
                context[CONTEXT - 1] = best as i64;
                decoded = decode(&context)?;
            }
        }
        let mut text = String::new();
        for &token in &found { text.push_str(self.model.tokens.get(token).map_or("", |s| s.as_str())); }
        Ok(text.replace('\u{2581}', " ").trim().into())
    }
}
