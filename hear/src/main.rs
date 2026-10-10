#![no_std]
#![no_main]
// hear [SECONDS] [--save FILE] | hear --wav FILE [--save FILE]: recognizes voice commands (docs/voice V1, issue 078).
// The model (voice/model.bin) and the grammar (voice/commands.txt) come from the boot disk; the grammar's phrases are
// spelled with the synthesizer's letter-to-sound rules (the phonetics crate). From the microphone it waits up to
// SECONDS (default 8) for one utterance, with a level meter on one line; from a file it recognizes every utterance in
// it. For the operator (000-APP-0053) each utterance gets its time, length and level and the three best phrases with
// their confidence, then `HEARD "открой файлы" INTENT=open TOOL=fm CONFIDENCE=0.91` or `NOT UNDERSTOOD (...)` and why;
// nothing heard gets the noise floor and the peak. `--save` keeps the first utterance, or the whole wait when nothing
// was heard, as a WAV file. Least authority (Art. 11.5, 11.11): it is lent the user's files for `--save` alone and drops
// them at start without it, and with it once the file is written, before anything is recognized; what it heard is
// printed, not acted on.
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use hear::Sound;
use mind::voice::recognizer::Recognizer;
use mind::abi::{SLOT_FILE, SLOT_VFS};
use mind::ipc::Endpoint;
use mind::voice::{meter, Detector, Utterance, RATE};

mind::request!(REQUEST_CONSOLE | REQUEST_FILES);

const USAGE: &str = "USAGE: HEAR [SECONDS] [--save FILE] | HEAR --wav FILE [--save FILE]";
// The level meter's line is written again this often (samples of 16 kHz: 250 ms).
const METER_EVERY: usize = RATE as usize / 4;
// The candidates shown for each utterance.
const CANDIDATES: usize = 3;

fn hundredths(per_mille: u32) -> String { alloc::format!("{}.{:02}", per_mille / 1000, per_mille % 1000 / 10) }

fn report(recognizer: &Recognizer, utterance: &Utterance) {
    mind::println!("UTTERANCE AT {}.{:02} S: {} MS, LEVEL {} DBFS", utterance.start_ms / 1000, utterance.start_ms % 1000 / 10, utterance.length_ms(), utterance.level);
    let Some((heard, candidates)) = recognizer.recognize_ranked(&utterance.samples, &|_| true, CANDIDATES) else { mind::println!("NOT UNDERSTOOD (TOO SHORT)"); return };
    let list: Vec<String> = candidates.iter().map(|&(i, c)| alloc::format!("\"{}\" {}", recognizer.grammar.phrases[i].text, hundredths(c))).collect();
    mind::println!("  CANDIDATES: {}", list.join(", "));
    let confidence = heard.decoded.confidence();
    let phrase = &recognizer.grammar.phrases[heard.decoded.phrase];
    let mut slots = String::new();
    for (name, value) in &phrase.slots { slots.push_str(&alloc::format!(" {}={}", name.to_uppercase(), value)); }
    if heard.accepted {
        mind::println!("HEARD \"{}\" INTENT={}{} CONFIDENCE={}", phrase.text, phrase.intent, slots, hundredths(confidence));
    } else {
        mind::println!("NOT UNDERSTOOD (CLOSEST \"{}\", CONFIDENCE={}): {}", phrase.text, hundredths(confidence), recognizer.refusal(&heard.decoded).unwrap_or("REFUSED"));
    }
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    mind::about!("hear — recognizes a spoken command offline (Russian and English) and prints what it heard.\nUsage: hear [seconds] [--save file] | hear --wav file [--save file]   (default: one utterance within 8 s from the microphone)\nFor each utterance: its length and level, the three closest phrases and the verdict; from the microphone a level meter\nwhile it listens, and the noise floor when nothing was heard. --save keeps the utterance (or the whole wait) as a WAV file.\nThe commands are in voice/commands.txt. It acts on nothing; voice control is the shell's voice on.");
    let words: Vec<&str> = mind::process::args_str().split_whitespace().collect();
    let (words, save) = match words.iter().position(|w| *w == "--save") {
        Some(at) if at + 1 < words.len() => ([&words[..at], &words[at + 2..]].concat(), Some(words[at + 1])),
        Some(_) => { mind::println!("{}", USAGE); return; }
        None => (words, None),
    };
    let (wav, seconds) = match words.as_slice() {
        [] => (None, 8),
        ["--wav", path] => (Some(*path), 0),
        [n] => match n.parse::<usize>() { Ok(n) if (1..=60).contains(&n) => (None, n), _ => { mind::println!("{}", USAGE); return; } },
        _ => { mind::println!("{}", USAGE); return; }
    };
    // The user's files only to keep a recording (the model comes through the boot disk's reader, SLOT_VFS).
    if save.is_none() { let _ = mind::ipc::drop_cap(SLOT_FILE); }
    let recognizer = match hear::load() { Ok(r) => r, Err(error) => { mind::println!("HEAR: {}", error); return; } };
    let mut sound = Sound::default();
    // The whole wait, for --save when nothing is heard.
    let mut wait: Vec<i16> = Vec::new();
    let heard = match wav {
        Some(path) => hear::wav_utterances(path, &mut |detector, chunk| sound.note(detector, chunk)),
        None => {
            mind::println!("LISTENING FOR {} S: SPEECH STARTS 9 DB ABOVE THE NOISE FLOOR, NOT BELOW -50 DBFS", seconds);
            let mut since = 0usize;
            let mut watch = |detector: &Detector, chunk: &[i16]| {
                sound.note(detector, chunk);
                if save.is_some() { wait.extend_from_slice(chunk); }
                since += chunk.len();
                if since >= METER_EVERY {
                    since = 0;
                    let (peak, rms) = meter(chunk);
                    mind::print!("\rLEVEL: PEAK {:>3} DBFS, RMS {:>3} DBFS; NOISE FLOOR {:>3} DBFS; SPEECH FROM {:>3} DBFS{}",
                                 peak, rms, detector.floor_dbfs(), detector.threshold_dbfs(), if detector.speaking() { "; SPEAKING" } else { "          " });
                }
            };
            let result = hear::listen_once(seconds, &mut watch);
            mind::println!("");
            result.map(|u| u.into_iter().collect::<Vec<_>>())
        }
    };
    let all = match heard { Ok(all) => all, Err(error) => { mind::println!("HEAR: {}", error); return; } };
    // Kept, and the user's files dropped, before the recognizer sees any of it.
    if let Some(path) = save {
        match (all.first(), wait.is_empty()) {
            (Some(first), _) => saved(path, &first.samples, "THE FIRST UTTERANCE"),
            (None, false) => saved(path, &wait, "THE WHOLE WAIT"),
            (None, true) => mind::println!("HEAR: NOTHING TO SAVE"),
        }
        let _ = mind::ipc::drop_cap(SLOT_FILE);
    }
    for u in &all { report(&recognizer, u); }
    if all.is_empty() { mind::println!("NOTHING HEARD: {}", sound.nothing()); }
}

// --save: the samples as a WAV file through the user's files, and what was kept.
fn saved(path: &str, samples: &[i16], what: &str) {
    mind::fs::use_endpoint(Endpoint(SLOT_FILE));
    match hear::save(path, samples) {
        Ok(()) => mind::println!("SAVED {} TO {}: {} MS, 16 KHZ MONO", what, path, samples.len() * 1000 / RATE as usize),
        Err(error) => mind::println!("HEAR: {}", error),
    }
    mind::fs::use_endpoint(Endpoint(SLOT_VFS));
}
