//! The voice command recognizer (docs/voice, V1): features, the acoustic model and the grammar together. An
//! utterance is understood when its best phrase fits nearly as well as any phone sequence (its mean deficit is at most
//! the model's `accept`) and clearly better than the best phrase meaning something else (`margin`). Host tests include
//! this file.
use super::features::{normalize, Features};
use super::grammar::{Decoded, Grammar, Phrase};
use super::model::Model;

pub struct Recognizer { pub features: Features, pub model: Model, pub grammar: Grammar, silence: u16 }

/// What was heard: the best phrase and whether it is accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recognition { pub decoded: Decoded, pub accepted: bool }

impl Recognition {
    /// The phrase when it is accepted.
    pub fn phrase(&self) -> Option<usize> { self.accepted.then_some(self.decoded.phrase) }
}

impl Recognizer {
    /// None if the model has no silence class (`sil`).
    pub fn new(model: Model, grammar: Grammar) -> Option<Self> {
        let silence = model.class("sil")? as u16;
        Some(Self { features: Features::new(), model, grammar, silence })
    }

    /// Recognizes 16 kHz mono speech (an utterance, with or without silence around it).
    pub fn recognize(&self, samples: &[i16]) -> Option<Recognition> { self.recognize_where(samples, &|_| true) }

    /// `recognize` among the phrases `keep` chooses (for instance only yes and no).
    pub fn recognize_where(&self, samples: &[i16], keep: &dyn Fn(&Phrase) -> bool) -> Option<Recognition> {
        let mut features = self.features.log_mel(samples);
        normalize(&mut features);
        let scores = self.model.scores(&features);
        let decoded = self.grammar.decode_where(&scores, self.model.classes.len(), self.silence, keep)?;
        Some(Recognition { decoded, accepted: decoded.deficit() <= self.model.accept && decoded.margin() >= self.model.margin })
    }
}
