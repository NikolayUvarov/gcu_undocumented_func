#![no_std]
#![no_main]
// dictate: speech to text with the dictation models (250). `dictate file.wav` reads a network file (convert.py's
// MINDNN01; by default the Russian model on the model disk), checks its checksum, and prints the text of a WAV
// (made 16 kHz mono) with the time each step took. A model on models: is used only if its SHA-256 is the one
// MANIFEST.json lists for it (MC-4.2). `dictate --features file.wav` prints Kaldi's log-mel filter bank,
// one line per frame, so the voice suite compares it with kaldi-native-fbank's. The text is only printed (MC-11.5).
// Built for x86_64 with SSE2 (targets/x86_64-mind-float.json); AVX2 where the processor has it.
extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::nn::{self, Model};
use mind::sha256::Sha256;
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
    let Some((words, bytes, hash)) = read_model(model) else { return };
    let read = mind::time::monotonic_ns();
    if let Some(rest) = model.strip_prefix("models:") {
        match listed_hash(rest) {
            Some(listed) if listed == hash => mind::println!("DICTATE: SHA256 MATCHES MANIFEST.JSON"),
            Some(_) => { mind::println!("dictate: {}: its SHA-256 is not the one MANIFEST.json lists; not used", model); return }
            None => { mind::println!("dictate: {}: not in MANIFEST.json; not used", model); return }
        }
    }
    // SAFETY: the words hold `bytes` bytes of the file; a byte view of u64s is always valid.
    let view = unsafe { core::slice::from_raw_parts(words.as_ptr() as *const u8, bytes) };
    let dictation = match Model::parse(view, true).and_then(Dictation::new) { Ok(d) => d, Err(error) => { mind::println!("dictate: {}: {:?}", model, error); return } };
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

// The file in 8-byte words (the weights are read in place), its length in bytes and its SHA-256 in hex.
fn read_model(path: &str) -> Option<(Vec<u64>, usize, [u8; 64])> {
    let mut file = match mind::fs::File::open(path) { Ok(f) => f, Err(error) => { mind::println!("dictate: {}: {:?}", path, error); return None } };
    let size = file.size();
    let mut words = vec![0u64; size.div_ceil(8)];
    // SAFETY: as above; the slice covers the words' first `size` bytes.
    let bytes = unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut u8, size) };
    let mut hash = Sha256::new();
    let mut at = 0;
    while at < size {
        let end = (at + mind::fs::CHUNK).min(size);
        match file.read(&mut bytes[at..end]) {
            Ok(0) => break,
            Ok(n) => { hash.update(&bytes[at..at + n]); at += n }
            Err(error) => { mind::println!("dictate: {}: {:?}", path, error); return None }
        }
    }
    if at < size { mind::println!("dictate: {}: {} of {} bytes", path, at, size); return None; }
    let mut hex = [0u8; 64];
    for (i, b) in hash.finish().iter().enumerate() { hex[2 * i] = b"0123456789abcdef"[(b >> 4) as usize]; hex[2 * i + 1] = b"0123456789abcdef"[(b & 15) as usize]; }
    Some((words, size, hex))
}

// The SHA-256 models:MANIFEST.json lists for `path` among the files added to the disk (scripts/models.py disk --add).
fn listed_hash(path: &str) -> Option<[u8; 64]> {
    let mut file = mind::fs::File::open("models:MANIFEST.json").ok()?;
    let mut text = vec![0u8; file.size().min(1 << 20)];
    let mut at = 0;
    while at < text.len() { match file.read(&mut text[at..]) { Ok(0) | Err(_) => break, Ok(n) => at += n } }
    let text = core::str::from_utf8(&text[..at]).ok()?;
    let added = &text[text.find("\"added\"")?..];
    let entry = &added[added.find(&alloc::format!("\"path\": \"{}\"", path))?..];
    let entry = &entry[..entry.find('}')?];
    let hex = entry[entry.find("\"sha256\": \"")? + 11..].get(..64)?;
    hex.as_bytes().try_into().ok()
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
