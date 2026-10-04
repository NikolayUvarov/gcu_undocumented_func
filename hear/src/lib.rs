#![no_std]
//! What `hear` and `voice` share (docs/voice, issues 078–079): reading the model and the grammar from the boot disk,
//! spelling the grammar's phrases with the synthesizer's letter-to-sound rules, and taking utterances from the
//! microphone or a WAV file.
extern crate alloc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use mind::voice::grammar::{Grammar, Token};
use mind::voice::model::Model;
use mind::voice::recognizer::Recognizer;
use mind::voice::{Detector, Microphone, Source, Stream, Utterance, Wav};
use phonetics::phonemes::{Ph, Unit};

pub const MODEL: &str = "voice/model.bin";
pub const GRAMMAR: &str = "voice/commands.txt";

pub fn read(path: &str) -> Result<Vec<u8>, mind::fs::Error> {
    let mut file = mind::fs::File::open(path)?;
    let mut bytes = vec![0u8; file.size()];
    let mut filled = 0;
    while filled < bytes.len() {
        let got = file.read(&mut bytes[filled..])?;
        if got == 0 { break; }
        filled += got;
    }
    bytes.truncate(filled);
    Ok(bytes)
}

/// A phrase in the model's classes, with a possible pause between words.
pub fn pronounce(model: &Model, text: &str) -> Option<Vec<Token>> {
    let mut units = vec![Unit { ph: Ph::Pause(0), soft: false, stress: false }; 1024];
    let count = phonetics::text::parse(text, &mut units);
    let mut out = Vec::new();
    for unit in &units[..count] {
        match unit.ph {
            Ph::Pause(0) => {}
            Ph::Pause(_) | Ph::End(_) => if !out.is_empty() && out.last() != Some(&Token::Pause) { out.push(Token::Pause) },
            ph => out.push(Token::Phone(model.class(ph.name()?)? as u16)),
        }
    }
    while out.last() == Some(&Token::Pause) { out.pop(); }
    Some(out)
}

/// The recognizer with the model and the grammar of the boot disk.
pub fn load() -> Result<Recognizer, String> {
    let bytes = read(MODEL).map_err(|e| alloc::format!("CANNOT READ {}: {:?}", MODEL, e))?;
    let model = Model::parse(&bytes).map_err(|e| alloc::format!("{}: {:?}", MODEL, e))?;
    let text = read(GRAMMAR).map_err(|e| alloc::format!("CANNOT READ {}: {:?}", GRAMMAR, e))?;
    let text = core::str::from_utf8(&text).map_err(|_| alloc::format!("{}: NOT UTF-8", GRAMMAR))?;
    let grammar = Grammar::parse(text, &mut |phrase| pronounce(&model, phrase)).map_err(|e| alloc::format!("{}: {:?}", GRAMMAR, e))?;
    Recognizer::new(model, grammar).ok_or_else(|| alloc::format!("{}: NO SILENCE CLASS", MODEL))
}

/// A capture error as text: the microphone may be another program's (audio.wit 1.1).
pub fn capture_error(error: mind::Error) -> String {
    if error == mind::Error::Other(mind::abi::ERR_BUSY) { String::from("THE MICROPHONE IS BUSY (ANOTHER PROGRAM RECORDS)") } else { alloc::format!("NO MICROPHONE: {:?}", error) }
}

/// The model was trained on phrases with 30–400 ms of silence around them (scripts/voice_train.rs) and the detector
/// cuts close to the sound, which costs short words their edges (the burst of «нет»'s «т»): an utterance goes to the
/// recognizer with up to this much of the stream before and after it.
pub const CONTEXT_MS: usize = 200;
const PER_MS: usize = mind::voice::RATE as usize / 1000;
/// How much of the stream is kept for that: more than the longest utterance and its context.
const KEEP: usize = 10_000 * PER_MS;

/// `u` widened by up to `CONTEXT_MS` on each side from `recent` (the stream from sample `base` on).
fn with_context(u: Utterance, recent: &[i16], base: usize) -> Utterance {
    let (first, last) = (u.start_ms as usize * PER_MS, u.start_ms as usize * PER_MS + u.samples.len());
    let start = first.saturating_sub(CONTEXT_MS * PER_MS).max(base);
    let end = (last + CONTEXT_MS * PER_MS).min(base + recent.len());
    if start > first || end < last { return u; }
    Utterance { start_ms: (start / PER_MS) as u64, samples: recent[start - base..end - base].to_vec(), level: u.level }
}

/// Feeds a stream to the speech detector and hands every utterance, with its context (`CONTEXT_MS`), to `found`
/// (which returns false to stop). `limit` ends it after that many 16 kHz samples (the microphone). Returns how many
/// utterances there were, or why the stream failed.
pub fn utterances<S: Source>(mut stream: Stream<S>, limit: Option<usize>, found: &mut dyn FnMut(Utterance) -> bool) -> Result<usize, String> where S::Error: core::fmt::Debug {
    let mut detector = Detector::new();
    let (mut chunk, mut count, mut total, mut idle) = (Vec::new(), 0usize, 0usize, 0usize);
    let mut ready: Vec<Utterance> = Vec::new();
    let (mut recent, mut base): (Vec<i16>, usize) = (Vec::new(), 0);
    loop {
        chunk.clear();
        stream.read(&mut chunk).map_err(|e| alloc::format!("READ FAILED: {:?}", e))?;
        total += chunk.len();
        recent.extend_from_slice(&chunk);
        if recent.len() > 2 * KEEP { let cut = recent.len() - KEEP; recent.drain(..cut); base += cut; }
        detector.push(&chunk, &mut |u| ready.push(u));
        for u in ready.drain(..) { count += 1; if !found(with_context(u, &recent, base)) { return Ok(count); } }
        if limit.is_some_and(|l| total >= l) || stream.finished() { break; }
        if chunk.is_empty() {
            idle += 1;
            if idle > 100 { return Err(String::from("NO INPUT FROM THE MICROPHONE")); } // 2 s without data
            mind::time::sleep(20);
        } else { idle = 0; }
    }
    detector.finish(&mut |u| ready.push(u));
    for u in ready.drain(..) { count += 1; if !found(with_context(u, &recent, base)) { break; } }
    Ok(count)
}

/// One utterance from the microphone within `seconds` (None: nothing was said).
pub fn listen_once(seconds: usize) -> Result<Option<Utterance>, String> {
    let microphone = Microphone::start().map_err(capture_error)?;
    let mut heard = None;
    utterances(Stream::new(microphone), Some(seconds * mind::voice::RATE as usize), &mut |u| { heard = Some(u); false })?;
    Ok(heard)
}

/// Every utterance of a WAV file.
pub fn wav_utterances(path: &str) -> Result<Vec<Utterance>, String> {
    let file = Wav::open(path).map_err(|e| alloc::format!("CANNOT READ {}: {:?}", path, e))?;
    let mut all = Vec::new();
    utterances(Stream::new(file), None, &mut |u| { all.push(u); true })?;
    Ok(all)
}
