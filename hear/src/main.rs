#![no_std]
#![no_main]
// hear [SECONDS] | hear --wav FILE: recognizes voice commands (docs/voice V1, issue 078). The model (voice/model.bin)
// and the grammar (voice/commands.txt) come from the boot disk through the program's read-only file client; the
// grammar's phrases are spelled with the synthesizer's letter-to-sound rules (the phonetics crate). From the microphone
// it waits up to SECONDS (default 8) for one utterance; from a file it recognizes every utterance in it. Each gives
// `HEARD "открой файлы" INTENT=open TOOL=fm CONFIDENCE=0.91` or `NOT UNDERSTOOD (...)`. The recognizer holds the audio
// and file clients only: what it heard is printed, not acted on (Art. 11.11).
extern crate alloc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use mind::voice::grammar::{Grammar, Token};
use mind::voice::model::Model;
use mind::voice::recognizer::Recognizer;
use mind::voice::{Detector, Microphone, Source, Stream, Utterance, Wav};
use phonetics::phonemes::{Ph, Unit};

mind::request!(REQUEST_CONSOLE);

const MODEL: &str = "voice/model.bin";
const GRAMMAR: &str = "voice/commands.txt";
const USAGE: &str = "USAGE: HEAR [SECONDS] | HEAR --wav FILE";

fn read(path: &str) -> Result<Vec<u8>, mind::fs::Error> {
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

// A phrase in the model's classes, with a possible pause between words.
fn pronounce(model: &Model, text: &str) -> Option<Vec<Token>> {
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

fn load() -> Result<Recognizer, String> {
    let bytes = read(MODEL).map_err(|e| alloc::format!("CANNOT READ {}: {:?}", MODEL, e))?;
    let model = Model::parse(&bytes).map_err(|e| alloc::format!("{}: {:?}", MODEL, e))?;
    let text = read(GRAMMAR).map_err(|e| alloc::format!("CANNOT READ {}: {:?}", GRAMMAR, e))?;
    let text = core::str::from_utf8(&text).map_err(|_| alloc::format!("{}: NOT UTF-8", GRAMMAR))?;
    let grammar = Grammar::parse(text, &mut |phrase| pronounce(&model, phrase)).map_err(|e| alloc::format!("{}: {:?}", GRAMMAR, e))?;
    Recognizer::new(model, grammar).ok_or_else(|| alloc::format!("{}: NO SILENCE CLASS", MODEL))
}

fn report(recognizer: &Recognizer, utterance: &Utterance) {
    let Some(heard) = recognizer.recognize(&utterance.samples) else { mind::println!("NOT UNDERSTOOD (TOO SHORT)"); return };
    let confidence = heard.decoded.confidence();
    let phrase = &recognizer.grammar.phrases[heard.decoded.phrase];
    let mut slots = String::new();
    for (name, value) in &phrase.slots { slots.push_str(&alloc::format!(" {}={}", name.to_uppercase(), value)); }
    if heard.accepted {
        mind::println!("HEARD \"{}\" INTENT={}{} CONFIDENCE={}.{:02}", phrase.text, phrase.intent, slots, confidence / 1000, confidence % 1000 / 10);
    } else {
        mind::println!("NOT UNDERSTOOD (CLOSEST \"{}\", CONFIDENCE={}.{:02})", phrase.text, confidence / 1000, confidence % 1000 / 10);
    }
}

/// Feeds the stream to the detector; `limit` stops after that many 16 kHz samples (the microphone), and `once` after
/// the first utterance. Returns how many utterances there were.
fn listen<S: Source>(recognizer: &Recognizer, mut stream: Stream<S>, limit: Option<usize>, once: bool) -> usize where S::Error: core::fmt::Debug {
    let mut detector = Detector::new();
    let (mut chunk, mut heard, mut total, mut idle) = (Vec::new(), 0usize, 0usize, 0usize);
    let mut found: Vec<Utterance> = Vec::new();
    loop {
        chunk.clear();
        if let Err(error) = stream.read(&mut chunk) { mind::println!("HEAR: READ FAILED: {:?}", error); break; }
        total += chunk.len();
        detector.push(&chunk, &mut |u| found.push(u));
        for u in found.drain(..) { report(recognizer, &u); heard += 1; }
        if (once && heard > 0) || limit.is_some_and(|l| total >= l) || stream.finished() { break; }
        if chunk.is_empty() {
            idle += 1;
            if idle > 100 { mind::println!("HEAR: NO INPUT FROM THE MICROPHONE"); return heard; } // 2 s without data
            mind::time::sleep(20);
        } else { idle = 0; }
    }
    if !(once && heard > 0) {
        detector.finish(&mut |u| found.push(u));
        for u in found.drain(..) { report(recognizer, &u); heard += 1; }
    }
    heard
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    let args = mind::process::args_str().trim();
    let (wav, seconds) = match args.split_whitespace().collect::<Vec<_>>().as_slice() {
        [] => (None, 8),
        ["--wav", path] => (Some(*path), 0),
        [n] => match n.parse::<usize>() { Ok(n) if (1..=60).contains(&n) => (None, n), _ => { mind::println!("{}", USAGE); return; } },
        _ => { mind::println!("{}", USAGE); return; }
    };
    let recognizer = match load() { Ok(r) => r, Err(error) => { mind::println!("HEAR: {}", error); return; } };
    let heard = match wav {
        Some(path) => match Wav::open(path) {
            Ok(file) => listen(&recognizer, Stream::new(file), None, false),
            Err(error) => { mind::println!("HEAR: CANNOT READ {}: {:?}", path, error); return; }
        },
        None => match Microphone::start() {
            Ok(microphone) => { mind::println!("LISTENING FOR {} S...", seconds); listen(&recognizer, Stream::new(microphone), Some(seconds * mind::voice::RATE as usize), true) }
            Err(error) => { mind::println!("HEAR: NO MICROPHONE: {:?}", error); return; }
        },
    };
    if heard == 0 { mind::println!("NOTHING HEARD"); }
}
