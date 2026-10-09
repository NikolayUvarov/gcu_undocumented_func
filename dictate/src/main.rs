#![no_std]
#![no_main]
// dictate: speech to text with the dictation models (250). Its first step is the models' features in the system:
// `dictate --features file.wav` prints Kaldi's log-mel filter bank of a WAV (made 16 kHz mono), one line per frame, so
// the voice suite compares it with kaldi-native-fbank's. Built for x86_64 with SSE2 (targets/x86_64-mind-float.json).
extern crate alloc;
use alloc::vec::Vec;
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::voice::fbank::{Fbank, BANDS};
use mind::voice::{Stream, Wav};

mind::request!(REQUEST_CONSOLE);

const USAGE: &str = "Usage: dictate --features file.wav";

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("dictate — speech to text (250); for now the models' features of a WAV.\nUsage: dictate --features file.wav\n--features: the 80 log-mel bands of every 10 ms, as Kaldi computes them.");
    let args = mind::process::args_str();
    let mut words = args.split_whitespace();
    let (Some("--features"), Some(path), None) = (words.next(), words.next(), words.next()) else { mind::println!("{}", USAGE); return };
    let wav = match Wav::open(path) { Ok(wav) => wav, Err(error) => { mind::println!("dictate: {}: {:?}", path, error); return } };
    let mut stream = Stream::new(wav);
    let mut samples: Vec<i16> = Vec::new();
    while !stream.finished() { if stream.read(&mut samples).unwrap_or(0) == 0 && stream.finished() { break; } }
    let start = mind::time::monotonic_ns();
    let fbank = Fbank::new();
    let features = fbank.compute_i16(&samples);
    let spent = mind::time::monotonic_ns() - start;
    let frames = features.len() / BANDS;
    mind::println!("FBANK SAMPLES={} FRAMES={} IN {} US", samples.len(), frames, spent / 1000);
    let mut line = alloc::string::String::new();
    for (frame, bands) in features.chunks_exact(BANDS).enumerate() {
        line.clear();
        let _ = write!(line, "F{}", frame);
        for value in bands { let _ = write!(line, " {:.5}", value); }
        mind::println!("{}", line);
    }
}
