#![no_std]
#![no_main]
// voice [--wav FILE] [SECONDS]: the voice side of voice control (docs/voice V2, issue 079). The shell starts it and
// lends it a client of idl/voice.wit in SLOT_INIT; voice asks the shell for its next order, speaks what the shell
// answers (through tts), listens when told to — up to SECONDS (default 6) from the microphone, or the next utterance of
// a WAV file standing in for it — recognizes the utterance (only yes or no when the shell asks for a confirmation) and
// reports it. The shell decides what a phrase does. voice drops the clients it does not need at start, keeping the
// audio, tts and read-only file clients and the line to the shell (Art. 11.11).
extern crate alloc;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{SLOT_INIT, SLOT_LOADER, SLOT_RTC};
use mind::idl::voice::{self, Action};
use mind::ipc::Endpoint;
use mind::voice::recognizer::Recognizer;
use mind::voice::Utterance;

mind::request!(REQUEST_CONSOLE);

const SHELL: Endpoint = Endpoint(SLOT_INIT);
const USAGE: &str = "USAGE: VOICE [--wav FILE] [SECONDS]";

/// Where utterances come from.
enum Input { Microphone(usize), Wav(VecDeque<Utterance>) }

/// What to tell the shell about an utterance.
#[derive(Default)]
struct Report { heard: bool, understood: bool, text: String, intent: String, slots: String, confidence: u16 }

fn recognize(recognizer: &Recognizer, utterance: &Utterance, yes_no: bool) -> Report {
    let closed = |p: &mind::voice::grammar::Phrase| matches!(p.intent.as_str(), "yes" | "no" | "cancel");
    let heard = if yes_no { recognizer.recognize_where(&utterance.samples, &closed) } else { recognizer.recognize(&utterance.samples) };
    let Some(heard) = heard else { return Report { heard: true, ..Report::default() } };
    let phrase = &recognizer.grammar.phrases[heard.decoded.phrase];
    let slots = phrase.slots.iter().map(|(name, value)| alloc::format!("{}={}", name, value)).collect::<Vec<_>>().join(" ");
    let confidence = heard.decoded.confidence().min(1000) as u16;
    mind::println!("[VOICE] HEARD \"{}\" INTENT={}{}{} CONFIDENCE={} {}", phrase.text, phrase.intent, if slots.is_empty() { "" } else { " " }, slots, confidence,
                   if heard.accepted { "ACCEPTED" } else { "REFUSED" });
    if !heard.accepted { return Report { heard: true, understood: false, text: phrase.text.clone(), confidence, ..Report::default() }; }
    Report { heard: true, understood: true, text: phrase.text.clone(), intent: phrase.intent.clone(), slots, confidence }
}

fn listen(input: &mut Input) -> Result<Option<Utterance>, String> {
    match input {
        Input::Microphone(seconds) => hear::listen_once(*seconds),
        Input::Wav(queue) => Ok(queue.pop_front()),
    }
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    // Least authority: voice needs neither the clock nor the loader.
    let _ = mind::ipc::drop_cap(SLOT_RTC);
    let _ = mind::ipc::drop_cap(SLOT_LOADER);
    let words: Vec<&str> = mind::process::args_str().split_whitespace().collect();
    let (wav, seconds) = match words.as_slice() {
        [] => (None, 6),
        ["--wav", path] => (Some(*path), 6),
        [n] => match n.parse::<usize>() { Ok(n) if (1..=30).contains(&n) => (None, n), _ => { mind::println!("{}", USAGE); return; } },
        _ => { mind::println!("{}", USAGE); return; }
    };
    let recognizer = match hear::load() { Ok(r) => r, Err(error) => { mind::println!("VOICE: {}", error); return; } };
    let mut input = match wav {
        Some(path) => match hear::wav_utterances(path) { Ok(all) => Input::Wav(all.into()), Err(error) => { mind::println!("VOICE: {}", error); return; } },
        None => Input::Microphone(seconds),
    };
    mind::println!("[VOICE] READY: {} PHRASES, {}", recognizer.grammar.phrases.len(), match &input { Input::Wav(q) => alloc::format!("{} UTTERANCES FROM {}", q.len(), wav.unwrap_or("")), Input::Microphone(s) => alloc::format!("MICROPHONE, {} S", s) });
    let mut report = Report::default();
    loop {
        let order = match voice::next(SHELL, report.heard, report.understood, &report.text, &report.intent, &report.slots, report.confidence) {
            Ok(order) => order,
            Err(error) => { mind::println!("[VOICE] THE SHELL IS GONE ({:?})", error); return; }
        };
        let say = order.say.as_str();
        if !say.is_empty() {
            mind::println!("[VOICE] SAY {}", say);
            // tts answers once the speech is queued: wait until it has been spoken (not to hear it, and so that the
            // shell may reboot at the next call).
            match mind::tts::say(say) { Ok(ms) => { let _ = mind::time::sleep(ms + 200); } Err(error) => mind::println!("[VOICE] CANNOT SPEAK ({:?})", error) }
        }
        report = Report::default();
        match order.action {
            Action::Wait => {}
            Action::Quit => { mind::println!("[VOICE] DONE"); return; }
            Action::Listen | Action::ListenYesNo => match listen(&mut input) {
                Ok(Some(utterance)) => report = recognize(&recognizer, &utterance, order.action == Action::ListenYesNo),
                Ok(None) => mind::println!("[VOICE] NOTHING HEARD"),
                Err(error) => mind::println!("[VOICE] {}", error),
            },
        }
    }
}
