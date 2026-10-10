//! The voice command recognizer (docs/voice, V1): features, the acoustic model and the grammar together. An
//! utterance is understood when its best phrase fits nearly as well as any phone sequence (its mean deficit is at most
//! the model's `accept`) and clearly better than the best phrase meaning something else (`margin`). Host tests include
//! this file.
use super::features::{normalize, Features};
use super::grammar::{confidence, Decoded, Grammar, Phrase};
use super::model::Model;
use alloc::vec::Vec;

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
        self.recognize_ranked(samples, keep, 0).map(|(recognition, _)| recognition)
    }

    /// `recognize_where`, and the best `count` phrases with their confidence in per mille, the best first
    /// (000-APP-0053: what `hear` shows the operator).
    pub fn recognize_ranked(&self, samples: &[i16], keep: &dyn Fn(&Phrase) -> bool, count: usize) -> Option<(Recognition, Vec<(usize, u32)>)> {
        let mut features = self.features.log_mel(samples);
        normalize(&mut features);
        let scores = self.model.scores(&features);
        let classes = self.model.classes.len();
        let ranked = self.grammar.rank_where(&scores, classes, self.silence, keep);
        let decoded = self.grammar.best(&ranked, scores.len() / classes)?;
        let candidates = ranked.iter().take(count).map(|&(phrase, score)| (phrase, confidence(score, decoded.frames))).collect();
        Some((Recognition { decoded, accepted: self.accepts(&decoded) }, candidates))
    }

    /// Whether a decoding is understood: near the best phone sequence and clear of other meanings.
    pub fn accepts(&self, decoded: &Decoded) -> bool { decoded.deficit() <= self.model.accept && decoded.margin() >= self.model.margin }

    /// Why a decoding is refused: too far from any phrase, or too close to one meaning something else.
    pub fn refusal(&self, decoded: &Decoded) -> Option<&'static str> {
        if decoded.deficit() > self.model.accept { Some("FAR FROM EVERY PHRASE") }
        else if decoded.margin() < self.model.margin { Some("TOO CLOSE TO ANOTHER MEANING") }
        else { None }
    }
}
