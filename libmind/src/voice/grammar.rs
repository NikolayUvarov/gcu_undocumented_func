//! The command grammar of the voice recognizer and its decoder (docs/voice, V1). `voice/commands.txt` lists intents and
//! their phrases with slots; every phrase, with its slots filled in, becomes a left-to-right chain of phone states
//! (each phone at least two frames, optional silence at the ends and between words). Viterbi decoding scores the
//! utterance against every chain; the model's scores are relative to each frame's best class, so a free loop of phones
//! (the filler for speech outside the grammar) scores 0, and a phrase's mean deficit per frame says how well it fits.
//! Host tests include this file.
use alloc::string::String;
use alloc::vec::Vec;

/// A phrase's sound: a phone (a class of the model) or a place between words where a pause may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token { Phone(u16), Pause }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phrase {
    /// The words, with the slots filled in.
    pub text: String,
    pub intent: String,
    /// (slot, value) for every slot of the phrase.
    pub slots: Vec<(String, String)>,
    pub tokens: Vec<Token>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarError {
    /// Line number and what is wrong with it.
    Syntax(usize, &'static str),
    UnknownSlot(usize, String),
    /// A phrase the pronunciation function could not spell.
    Unpronounceable(String),
    TooLarge,
}

/// At most this many phrases once the slots are filled in.
pub const MAX_PHRASES: usize = 2000;

pub struct Grammar { pub phrases: Vec<Phrase> }

struct Slot { name: String, words: Vec<(String, String)> }

impl Grammar {
    /// Reads the grammar; `pronounce` spells a phrase's text in tokens (None: it cannot).
    pub fn parse(text: &str, pronounce: &mut dyn FnMut(&str) -> Option<Vec<Token>>) -> Result<Self, GrammarError> {
        let mut slots: Vec<Slot> = Vec::new();
        let mut phrases = Vec::new();
        for (number, line) in text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())) {
            if line.is_empty() || line.starts_with('#') { continue; }
            let (head, body) = line.split_once(':').ok_or(GrammarError::Syntax(number, "expected NAME: ..."))?;
            let head = head.trim();
            if let Some(name) = head.strip_prefix("slot ") {
                let words = body.split(',').map(|w| match w.split_once('=') {
                    Some((words, value)) => (String::from(words.trim()), String::from(value.trim())),
                    None => (String::from(w.trim()), String::from(w.trim())),
                }).filter(|(w, _)| !w.is_empty()).collect::<Vec<_>>();
                if words.is_empty() { return Err(GrammarError::Syntax(number, "a slot without words")); }
                slots.push(Slot { name: String::from(name.trim()), words });
                continue;
            }
            if head.is_empty() || head.contains(char::is_whitespace) { return Err(GrammarError::Syntax(number, "an intent is one word")); }
            for pattern in body.split('|').map(str::trim).filter(|p| !p.is_empty()) {
                // Fill the slots in every combination.
                let mut partial: Vec<(String, Vec<(String, String)>)> = alloc::vec![(String::new(), Vec::new())];
                let mut rest = pattern;
                while !rest.is_empty() {
                    let (literal, slot, after) = match rest.find('{') {
                        Some(open) => {
                            let close = rest[open..].find('}').ok_or(GrammarError::Syntax(number, "unclosed {"))? + open;
                            (&rest[..open], Some(&rest[open + 1..close]), &rest[close + 1..])
                        }
                        None => (rest, None, ""),
                    };
                    for (text, _) in partial.iter_mut() { text.push_str(literal); }
                    if let Some(name) = slot {
                        let slot = slots.iter().find(|s| s.name == name.trim()).ok_or_else(|| GrammarError::UnknownSlot(number, String::from(name)))?;
                        let mut next = Vec::new();
                        for (text, values) in &partial {
                            for (words, value) in &slot.words {
                                let mut values = values.clone();
                                values.push((slot.name.clone(), value.clone()));
                                next.push((alloc::format!("{}{}", text, words), values));
                            }
                        }
                        if next.len() + phrases.len() > MAX_PHRASES { return Err(GrammarError::TooLarge); }
                        partial = next;
                    }
                    rest = after;
                }
                for (text, slots) in partial {
                    let text = String::from(text.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
                    let tokens = pronounce(&text).filter(|t| t.iter().any(|t| matches!(t, Token::Phone(_)))).ok_or_else(|| GrammarError::Unpronounceable(text.clone()))?;
                    phrases.push(Phrase { text, intent: String::from(head), slots, tokens });
                }
            }
        }
        if phrases.len() > MAX_PHRASES { return Err(GrammarError::TooLarge); }
        Ok(Self { phrases })
    }

    /// Decodes an utterance: `scores` holds `classes` scores per frame (0 for each frame's best class, see
    /// `Model::scores`), `silence` is the silence class. None without frames or phrases.
    pub fn decode(&self, scores: &[i32], classes: usize, silence: u16) -> Option<Decoded> { self.decode_where(scores, classes, silence, &|_| true) }

    /// `decode` among the phrases `keep` chooses (a closed grammar, such as yes or no for a confirmation).
    pub fn decode_where(&self, scores: &[i32], classes: usize, silence: u16, keep: &dyn Fn(&Phrase) -> bool) -> Option<Decoded> {
        let frames = scores.len() / classes;
        let mut results: Vec<(usize, i32)> = self.phrases.iter().enumerate().filter(|(_, p)| keep(p)).map(|(i, p)| (i, viterbi(&states(&p.tokens, silence), scores, classes, frames))).collect();
        if frames == 0 || results.is_empty() { return None; }
        results.sort_by(|a, b| b.1.cmp(&a.1));
        let (best, score) = results[0];
        let same = |i: usize| self.phrases[i].intent == self.phrases[best].intent && self.phrases[i].slots == self.phrases[best].slots;
        let second = results.iter().find(|&&(i, _)| !same(i)).copied();
        Some(Decoded { phrase: best, score, frames, second })
    }
}

/// The result of decoding: the best phrase, its score (1/256 nats, at most 0: the filler's) over `frames` frames, and
/// the best phrase meaning something else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded { pub phrase: usize, pub score: i32, pub frames: usize, pub second: Option<(usize, i32)> }

impl Decoded {
    /// The best phrase's mean deficit per frame against the filler, in 1/256 nats (0: as good as any phone sequence).
    pub fn deficit(&self) -> i32 { (-(self.score as i64) / self.frames.max(1) as i64) as i32 }
    /// How much better the best phrase fits than the best one meaning something else, per frame (1/256 nats).
    pub fn margin(&self) -> i32 { self.second.map_or(i32::MAX, |(_, s)| ((self.score as i64 - s as i64) / self.frames.max(1) as i64) as i32) }
    /// Confidence in per mille: e^(-deficit).
    pub fn confidence(&self) -> u32 { (1000.0 * super::features::exp(-(self.deficit() as f64) / 256.0) + 0.5) as u32 }
}

/// A state of a phrase's chain: its class, whether it may stay (self-loop) and whether it may be skipped.
#[derive(Clone, Copy)]
struct State { class: u16, stay: bool, skip: bool }

fn states(tokens: &[Token], silence: u16) -> Vec<State> {
    let pause = State { class: silence, stay: true, skip: true };
    let mut out = alloc::vec![pause];
    for token in tokens {
        match *token {
            Token::Phone(class) => { out.push(State { class, stay: false, skip: false }); out.push(State { class, stay: true, skip: false }); }
            Token::Pause => out.push(pause),
        }
    }
    out.push(pause);
    out
}

const NONE: i32 = i32::MIN / 4;

// The best path through the chain over all frames (higher is better).
fn viterbi(states: &[State], scores: &[i32], classes: usize, frames: usize) -> i32 {
    let n = states.len();
    let emit = |t: usize, s: &State| scores[t * classes + s.class as usize];
    let mut previous = alloc::vec![NONE; n];
    let mut current = alloc::vec![NONE; n];
    // At the first frame a path may start in any state that only skippable states precede.
    for (i, s) in states.iter().enumerate() {
        previous[i] = emit(0, s);
        if !s.skip { break; }
    }
    for t in 1..frames {
        let mut carry = NONE; // the best way into state i from earlier states
        for i in 0..n {
            if i > 0 { carry = previous[i - 1].max(if states[i - 1].skip { carry } else { NONE }); }
            let stay = if states[i].stay { previous[i] } else { NONE };
            let best = stay.max(carry);
            current[i] = if best == NONE { NONE } else { best + emit(t, &states[i]) };
        }
        core::mem::swap(&mut previous, &mut current);
    }
    // A path may end in any state that only skippable states follow.
    let mut best = NONE;
    for i in (0..n).rev() {
        best = best.max(previous[i]);
        if !states[i].skip { break; }
    }
    best
}
