//! The neural voice for Russian (252-APP-0128): Vosk TTS 0.7 from the model disk through `mind::voice::synthesis`, as
//! `speak` runs it. Loaded at the first Russian sentence when init lends a read-only scope of `models:` in SLOT_FILE and
//! the quota holds it; otherwise the formant voice speaks, and the log says why once.
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{AUDIO_RATE, CAP_KIND_ENDPOINT, SLOT_FILE};
use mind::ipc::Endpoint;
use mind::nn::Model;
use mind::voice::russian::Dictionary;
use mind::voice::synthesis::{self, Settings, Voice};
use mind::voice::Resampler;

pub const VOICE: &str = "models:tts-ru-vosk-0.7/voice.bin";
pub const DICTIONARY: &str = "models:tts-ru-vosk-0.7/russian.dic";
/// The male voice the maintainer chose for the system (252): Vosk TTS 0.7's speaker 3.
pub const SPEAKER: i64 = 3;
/// What the voice needs while it speaks beside its two files, probed with them before they are read: a heap
/// allocation that fails would end the service.
const WORKSPACE: usize = 96 << 20;

pub enum Neural { Untried, Ready(Voice<'static>), Absent }

impl Neural {
    /// The voice, loaded at the first call; None, said once, when it cannot be.
    pub fn voice(&mut self) -> Option<&Voice<'static>> {
        if matches!(self, Neural::Untried) {
            *self = match load() {
                Ok(voice) => Neural::Ready(voice),
                Err(why) => { mind::println!("[TTS] THE FORMANT VOICE SPEAKS RUSSIAN: {}", why); Neural::Absent }
            };
        }
        match self { Neural::Ready(voice) => Some(voice), _ => None }
    }
}

// Both files read and checked against the model disk's MANIFEST.json (mind::model_file), kept for the service's life.
fn load() -> Result<Voice<'static>, String> {
    if mind::dev::cap_info(SLOT_FILE).0 != CAP_KIND_ENDPOINT { return Err(String::from("NO SCOPE OF models: (THE MODEL DISK, AND INIT'S GRANT OF 252-APP-0128)")); }
    mind::fs::use_scope(Endpoint(SLOT_FILE), "models:");
    let size = |path: &str| mind::fs::File::open(path).map(|f| f.size()).map_err(|e| format!("{}: {:?}", path, e));
    let need = size(VOICE)? + size(DICTIONARY)? + WORKSPACE;
    if mind::mem::Pages::new(need).is_none() { return Err(format!("{} MIB DO NOT FIT ITS QUOTA (A LINE TTS <MIB> IN data/memory.conf)", need >> 20)); }
    let start = mind::time::monotonic_ns();
    let dictionary_file: &'static mind::model_file::File = Box::leak(Box::new(mind::model_file::read(DICTIONARY)?));
    let dictionary = Dictionary::parse(dictionary_file.bytes(), true).map_err(|e| format!("{}: {}", DICTIONARY, e))?;
    let model_file: &'static mind::model_file::File = Box::leak(Box::new(mind::model_file::read(VOICE)?));
    let voice = Model::parse(model_file.bytes(), true).and_then(|m| Voice::new(m, dictionary)).map_err(|e| format!("{}: {:?}", VOICE, e))?;
    mind::println!("[TTS] THE NEURAL VOICE SPEAKS RUSSIAN: {} BYTES OF VOICE, {} WORDS, READY IN {} MS", model_file.len(), voice.dictionary().words(),
        (mind::time::monotonic_ns() - start) / 1_000_000);
    Ok(voice)
}

/// One sentence at AUDIO_RATE, mono, appended to `out`; `rate` in per cent of the voice's own.
pub fn say(voice: &Voice<'static>, sentence: &str, rate: i64, out: &mut Vec<i16>) -> Result<(), String> {
    let settings = Settings { speaker: SPEAKER, length: 100.0 / rate.clamp(50, 200) as f32, ..Settings::default() };
    let audio = voice.say(sentence, &settings).map_err(|e| format!("{:?}", e))?;
    let pcm: Vec<i16> = audio.iter().map(|&a| (a * 32767.0).clamp(-32767.0, 32767.0) as i16).collect();
    Resampler::between(synthesis::RATE, AUDIO_RATE as u32, 1).process(&pcm, out);
    Ok(())
}

/// The text's sentences, each with whether the neural voice takes it: one with a Cyrillic letter.
pub fn sentences(text: &str) -> Vec<(&str, bool)> { synthesis::sentences(text).into_iter().map(|s| (s, crate::route::russian(s))).collect() }
