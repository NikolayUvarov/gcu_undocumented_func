#![no_std]
#![no_main]
// dictate: speech to text with the dictation models (250). `dictate file.wav` reads a network file (convert.py's
// MINDNN01; by default the Russian model on the model disk), checks its checksum, and prints the text of a WAV
// (made 16 kHz mono) with the time each step took. A model on models: is used only if its SHA-256 is the one
// MANIFEST.json lists for it (MC-4.2). `dictate --features file.wav` prints Kaldi's log-mel filter bank,
// one line per frame, so the voice suite compares it with kaldi-native-fbank's. The text is only printed (MC-11.5).
// Built for x86_64 with SSE2 (targets/x86_64-mind-float.json); AVX2 where the processor has it.
extern crate alloc;
use alloc::vec::Vec;
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::nn::{self, Model};
use mind::voice::dictation::Dictation;
use mind::voice::fbank::{Fbank, BANDS};
use mind::voice::{Stream, Wav};

// The model (71 MB), the features and the encoder's values of a long utterance.
mind::request!(REQUEST_CONSOLE, memory: 384);

const USAGE: &str = "Usage: dictate [--model FILE] file.wav | dictate --features file.wav";
/// The Russian model as `scripts/models.py disk --add` puts it on the model disk.
const MODEL: &str = "models:asr-ru-vosk-0.54/dictate.bin";

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("dictate — speech to text (250).\nUsage: dictate [--model FILE] file.wav\n       dictate --features file.wav\n--model: a network file from scripts/voice_dictate/convert.py (default models:asr-ru-vosk-0.54/dictate.bin).\n--features: the 80 log-mel bands of every 10 ms, as Kaldi computes them.");
    let args = mind::process::args_str();
    let words: Vec<&str> = args.split_whitespace().collect();
    let (features, model, wav) = match words[..] {
        ["--features", wav] => (true, MODEL, wav),
        ["--model", model, wav] => (false, model, wav),
        [wav] if !wav.starts_with("--") => (false, MODEL, wav),
        _ => { mind::println!("{}", USAGE); return }
    };
    let samples = match read_wav(wav) { Ok(s) => s, Err(error) => { mind::println!("dictate: {}: {:?}", wav, error); return } };
    if features { print_features(&samples); return; }
    let start = mind::time::monotonic_ns();
    let file = match mind::models::read(model) { Ok(f) => f, Err(error) => { mind::println!("dictate: {}; not used", error); return } };
    let read = mind::time::monotonic_ns();
    if model.starts_with("models:") { mind::println!("DICTATE: SHA256 MATCHES MANIFEST.JSON"); }
    let bytes = file.len();
    let dictation = match Model::parse(file.bytes(), true).and_then(Dictation::new) { Ok(d) => d, Err(error) => { mind::println!("dictate: {}: {:?}", model, error); return } };
    let parsed = mind::time::monotonic_ns();
    mind::println!("DICTATE: MODEL {} BYTES, READ IN {} MS, CHECKED IN {} MS; SIMD {}", bytes, (read - start) / 1_000_000, (parsed - read) / 1_000_000, if nn::gemm::simd(None) { "AVX2" } else { "NONE" });
    let text = dictation.text(&samples);
    let done = mind::time::monotonic_ns();
    match text {
        Ok(text) => {
            mind::println!("DICTATE: {} MS OF SPEECH IN {} MS", samples.len() as u64 * 1000 / 16_000, (done - parsed) / 1_000_000);
            mind::println!("TEXT: {}", text);
        }
        Err(error) => mind::println!("dictate: {:?}", error),
    }
}

// The WAV's samples, 16 kHz mono.
fn read_wav(path: &str) -> Result<Vec<i16>, mind::voice::OpenError> {
    let mut stream = Stream::new(Wav::open(path)?);
    let mut samples: Vec<i16> = Vec::new();
    while !stream.finished() { if stream.read(&mut samples).unwrap_or(0) == 0 && stream.finished() { break; } }
    Ok(samples)
}

fn print_features(samples: &[i16]) {
    let start = mind::time::monotonic_ns();
    let features = Fbank::new().compute_i16(samples);
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
