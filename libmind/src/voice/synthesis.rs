//! Speech synthesis with a VITS voice in a network file (252): Vosk TTS 0.7's Russian voices. Text to phoneme ids
//! (`russian`), the graph `vits` over them, 22.05 kHz mono audio. Host tests include this file.
use super::russian::Dictionary;
use crate::nn::{op_error, Data, Model, Result, Tensor};
use alloc::vec;
use alloc::vec::Vec;

/// The voices' sample rate.
pub const RATE: u32 = 22_050;

/// How a sentence is said: the model's noise, how long sounds last (1: the voice's pace), the noise in their lengths,
/// and the speaker.
#[derive(Clone, Copy, Debug)]
pub struct Settings { pub noise: f32, pub length: f32, pub length_noise: f32, pub speaker: i64 }

/// vosk-model-tts-ru-0.7's config.json.
impl Default for Settings { fn default() -> Self { Self { noise: 0.5, length: 1.0, length_noise: 0.8, speaker: 0 } } }

/// A voice: its network and its dictionary.
pub struct Voice<'f> { model: Model<'f>, dictionary: Dictionary<'f> }

impl<'f> Voice<'f> {
    /// Refuses a file without the graph `vits`.
    pub fn new(model: Model<'f>, dictionary: Dictionary<'f>) -> Result<Self> {
        if !model.has("vits") { return Err(op_error("no graph vits")); }
        Ok(Self { model, dictionary })
    }

    pub fn dictionary(&self) -> &Dictionary<'f> { &self.dictionary }

    /// The audio of `text`, in [-1, 1].
    pub fn say(&self, text: &str, settings: &Settings) -> Result<Vec<f32>> {
        let ids = self.dictionary.ids(text);
        if ids.len() < 4 { return Ok(Vec::new()); } // "^", "$" and the blank: nothing to say
        let n = ids.len();
        let out = self.model.run("vits", vec![
            Tensor::i64(vec![1, n], ids),
            Tensor::i64(vec![1], vec![n as i64]),
            Tensor::f32(vec![3], vec![settings.noise, settings.length, settings.length_noise]),
            Tensor::i64(vec![1], vec![settings.speaker]),
        ])?;
        match out.into_iter().next() { Some(Tensor { data: Data::F32(audio), .. }) => Ok(audio), _ => Err(op_error("the voice made no audio")) }
    }
}

/// A text's sentences, each with what ends it (. ! ? …), so that a long text is said one sentence at a time.
pub fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if matches!(c, '.' | '!' | '?' | '…') && chars.peek().is_none_or(|&(_, next)| next.is_whitespace()) {
            let end = at + c.len_utf8();
            if !text[start..end].trim().is_empty() { out.push(text[start..end].trim()); }
            start = end;
        }
    }
    if !text[start..].trim().is_empty() { out.push(text[start..].trim()); }
    out
}
