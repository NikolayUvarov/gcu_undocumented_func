#![no_std]
#![no_main]
// speak: Russian text said by a neural voice (252): Vosk TTS 0.7 from the model disk, a sentence at a time, played
// through the audio gateway (resampled to 48 kHz) or written to a WAV. The voice and its dictionary are used only if
// their SHA-256 are the ones the model disk's MANIFEST.json lists (MC-4.2). `--ids` prints the phoneme ids only.
// Built for x86_64 with SSE2 (targets/x86_64-mind-float.json), AVX2 where the processor has it; in soft float on aarch64.
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::BootInfo;
use mind::nn::{self, Model};
use mind::voice::russian::Dictionary;
use mind::voice::synthesis::{self, Settings, Voice};
use mind::voice::Resampler;

// The voice (146 MB), its dictionary (13 MB) and the network's values for a sentence.
mind::request!(REQUEST_CONSOLE, memory: 384);

const USAGE: &str = "Usage: speak [--speaker N] [--wav FILE] [--model FILE] [--dictionary FILE] [--ids] (text... | --file FILE)";
const MODEL: &str = "models:tts-ru-vosk-0.7/voice.bin";
const DICTIONARY: &str = "models:tts-ru-vosk-0.7/russian.dic";

struct Options<'a> { speaker: i64, wav: Option<&'a str>, model: &'a str, dictionary: &'a str, ids: bool, file: Option<&'a str>, text: String }

fn options(args: &str) -> Option<Options<'_>> {
    let mut o = Options { speaker: 0, wav: None, model: MODEL, dictionary: DICTIONARY, ids: false, file: None, text: String::new() };
    let mut words = args.split_whitespace();
    while let Some(w) = words.next() {
        match w {
            "--speaker" => o.speaker = words.next()?.parse().ok()?,
            "--wav" => o.wav = Some(words.next()?),
            "--model" => o.model = words.next()?,
            "--dictionary" => o.dictionary = words.next()?,
            "--ids" => o.ids = true,
            "--file" => o.file = Some(words.next()?),
            _ => { if !o.text.is_empty() { o.text.push(' '); } o.text.push_str(w); }
        }
    }
    if o.text.is_empty() == o.file.is_none() { None } else { Some(o) }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("speak — Russian text said by a neural voice (252).\nUsage: speak [--speaker N] [--wav FILE] [--model FILE] [--dictionary FILE] [--ids] (text... | --file FILE)\n--speaker: the voice's speaker (Vosk TTS 0.7: 0 a woman, 3 a man).\n--wav: write the speech to FILE (22.05 kHz) instead of playing it.\n--file: say the text of a UTF-8 file.\n--ids: print the phoneme ids only.\nThe voice and dictionary are on the model disk (models:tts-ru-vosk-0.7/).");
    let args = mind::process::args_str();
    let Some(mut o) = options(&args) else { mind::println!("{}", USAGE); return };
    if let Some(path) = o.file {
        match read_text(path) { Ok(text) => o.text = text, Err(e) => { mind::println!("speak: {}: {:?}", path, e); return } }
    }
    let start = mind::time::monotonic_ns();
    let dictionary_file = match mind::models::read(o.dictionary) { Ok(f) => f, Err(e) => { mind::println!("speak: {}; not used", e); return } };
    let dictionary = match Dictionary::parse(dictionary_file.bytes(), true) { Ok(d) => d, Err(e) => { mind::println!("speak: {}: {}", o.dictionary, e); return } };
    if o.ids {
        let ids = dictionary.ids(&o.text);
        let mut line = String::new();
        for id in ids { if !line.is_empty() { line.push(' '); } line.push_str(&alloc::format!("{}", id)); }
        mind::println!("IDS: {}", line);
        return;
    }
    let model_file = match mind::models::read(o.model) { Ok(f) => f, Err(e) => { mind::println!("speak: {}; not used", e); return } };
    let voice = match Model::parse(model_file.bytes(), true).and_then(|m| Voice::new(m, dictionary)) { Ok(v) => v, Err(e) => { mind::println!("speak: {}: {:?}", o.model, e); return } };
    let ready = mind::time::monotonic_ns();
    mind::println!("SPEAK: VOICE {} BYTES, DICTIONARY {} WORDS, READY IN {} MS; SIMD {}", model_file.len(), voice.dictionary().words(), (ready - start) / 1_000_000, if nn::gemm::simd(None) { "AVX2" } else { "NONE" });
    let settings = Settings { speaker: o.speaker, ..Settings::default() };
    let playing = o.wav.is_none() && mind::audio::info().map(|i| i.present).unwrap_or(false);
    if o.wav.is_none() && !playing { mind::println!("speak: no audio device; use --wav FILE"); return; }
    let mut resampler = Resampler::between(synthesis::RATE, mind::abi::AUDIO_RATE as u32, 1);
    let (mut written, mut spent, mut said) = (Vec::<i16>::new(), 0u64, 0usize);
    for sentence in synthesis::sentences(&o.text) {
        let t = mind::time::monotonic_ns();
        let audio = match voice.say(sentence, &settings) { Ok(a) => a, Err(e) => { mind::println!("speak: {:?}", e); return } };
        spent += mind::time::monotonic_ns() - t;
        said += audio.len();
        let pcm: Vec<i16> = audio.iter().map(|&a| (a * 32767.0).clamp(-32767.0, 32767.0) as i16).collect();
        if playing {
            let mut mono = Vec::new();
            resampler.process(&pcm, &mut mono);
            let stereo: Vec<i16> = mono.iter().flat_map(|&s| [s, s]).collect();
            if let Err(e) = mind::audio::play_all(&stereo) { mind::println!("speak: audio: {:?}", e); return; }
        } else {
            written.extend_from_slice(&pcm);
        }
    }
    mind::println!("SPEAK: {} MS OF SPEECH IN {} MS", said as u64 * 1000 / synthesis::RATE as u64, spent / 1_000_000);
    if let Some(path) = o.wav {
        match write_wav(path, &written) { Ok(()) => mind::println!("SPEAK: WROTE {}", path), Err(e) => mind::println!("speak: {}: {:?}", path, e) }
    }
}

// A UTF-8 text file (at most 1 MiB).
fn read_text(path: &str) -> Result<String, mind::fs::Error> {
    let mut file = mind::fs::File::open(path)?;
    let mut bytes = alloc::vec![0u8; file.size().min(1 << 20)];
    let mut at = 0;
    while at < bytes.len() { match file.read(&mut bytes[at..])? { 0 => break, n => at += n } }
    bytes.truncate(at);
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

// A 16-bit mono WAV at the voice's rate.
fn write_wav(path: &str, samples: &[i16]) -> Result<(), mind::fs::Error> {
    let data = samples.len() as u32 * 2;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    for v in [16u32] { bytes.extend_from_slice(&v.to_le_bytes()); }
    for v in [1u16, 1] { bytes.extend_from_slice(&v.to_le_bytes()); }
    for v in [synthesis::RATE, synthesis::RATE * 2] { bytes.extend_from_slice(&v.to_le_bytes()); }
    for v in [2u16, 16] { bytes.extend_from_slice(&v.to_le_bytes()); }
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for s in samples { bytes.extend_from_slice(&s.to_le_bytes()); }
    let mut file = mind::fs::File::create(path)?;
    let mut at = 0;
    while at < bytes.len() { at += file.write(&bytes[at..])?; }
    Ok(())
}
