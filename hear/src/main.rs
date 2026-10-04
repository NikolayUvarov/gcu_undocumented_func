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
use alloc::vec::Vec;
use mind::voice::recognizer::Recognizer;
use mind::voice::Utterance;

mind::request!(REQUEST_CONSOLE);

const USAGE: &str = "USAGE: HEAR [SECONDS] | HEAR --wav FILE";

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

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    let args = mind::process::args_str().trim();
    let (wav, seconds) = match args.split_whitespace().collect::<Vec<_>>().as_slice() {
        [] => (None, 8),
        ["--wav", path] => (Some(*path), 0),
        [n] => match n.parse::<usize>() { Ok(n) if (1..=60).contains(&n) => (None, n), _ => { mind::println!("{}", USAGE); return; } },
        _ => { mind::println!("{}", USAGE); return; }
    };
    let recognizer = match hear::load() { Ok(r) => r, Err(error) => { mind::println!("HEAR: {}", error); return; } };
    let heard = match wav {
        Some(path) => hear::wav_utterances(path).map(|all| { for u in &all { report(&recognizer, u); } all.len() }),
        None => {
            mind::println!("LISTENING FOR {} S...", seconds);
            hear::listen_once(seconds).map(|u| u.map_or(0, |u| { report(&recognizer, &u); 1 }))
        }
    };
    match heard {
        Ok(0) => mind::println!("NOTHING HEARD"),
        Ok(_) => {}
        Err(error) => mind::println!("HEAR: {}", error),
    }
}
