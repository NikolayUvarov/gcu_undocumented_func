//! Host tests of the Russian front end of the Vosk TTS voices (libmind/src/voice/russian.rs, 252): vosk-tts's
//! letter-to-sound rules on words worked out by hand, a small MINDDIC1 dictionary written here as
//! scripts/voice_tts/dictionary.py writes one, and, when MIND_TTS_DICTIONARY names the converted dictionary and
//! MIND_TTS_IDS the file scripts/voice_tts/vosk_ids.py wrote, the ids of the 24 sentences against vosk-tts's.
extern crate alloc;
#[path = "../libmind/src/voice/russian.rs"]
mod russian;

fn names(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

#[test]
fn letter_to_sound_rules() {
    // vosk_tts/g2p.py's own examples, and soft consonants, j after vowels and signs, ё, signs dropped.
    assert_eq!(russian::rules("абстракцион+истов"), names(&["a0", "b", "s", "t", "r", "a0", "k", "c", "i0", "o0", "nj", "i1", "s", "t", "o0", "v"]));
    assert_eq!(russian::rules("абстр+акция"), names(&["a0", "b", "s", "t", "r", "a1", "k", "c", "i0", "j", "a0"]));
    assert_eq!(russian::rules("ёлка"), names(&["j", "o0", "l", "k", "a0"]));
    assert_eq!(russian::rules("сел"), names(&["sj", "e0", "l"]));
    assert_eq!(russian::rules("съел"), names(&["s", "j", "e0", "l"]));
    assert_eq!(russian::rules("мать"), names(&["m", "a0", "tj"]));
    assert_eq!(russian::rules("щука-рыба"), names(&["sch", "u0", "k", "a0", "r", "y0", "b", "a0"]));
}

// A dictionary as dictionary.py writes one: words sorted in Windows-1251, 64 to a block, front coded.
fn build(phonemes: &[(&str, u8)], words: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let encode = |w: &str| -> Vec<u8> { w.chars().map(|c| match c { 'а'..='я' => (c as u32 - 'а' as u32 + 0xE0) as u8, 'ё' => 0xB8, _ => c as u8 }).collect() };
    let mut entries: Vec<(Vec<u8>, Vec<u8>)> = words.iter().map(|(w, tail)| (encode(w), tail.clone())).collect();
    entries.sort();
    let (mut offsets, mut data) = (Vec::new(), Vec::new());
    for block in entries.chunks(64) {
        offsets.push(data.len() as u32);
        let mut prev: &[u8] = &[];
        for (key, tail) in block {
            let shared = prev.iter().zip(key).take_while(|(a, b)| a == b).count();
            data.push(shared as u8);
            data.push((key.len() - shared) as u8);
            data.extend_from_slice(&key[shared..]);
            data.extend_from_slice(tail);
            prev = key;
        }
    }
    let mut out = b"MINDDIC1".to_vec();
    for v in [1u32, entries.len() as u32, offsets.len() as u32, phonemes.len() as u32] { out.extend_from_slice(&v.to_le_bytes()); }
    for (name, id) in phonemes { out.push(name.len() as u8); out.extend_from_slice(name.as_bytes()); out.push(*id); }
    for o in &offsets { out.extend_from_slice(&o.to_le_bytes()); }
    out.extend_from_slice(&data);
    let sum = out.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

#[test]
fn dictionary_and_ids() {
    let table: Vec<(&str, u8)> = vec![("_", 0), ("^", 1), ("$", 2), (" ", 3), (",", 8), (".", 10), ("a0", 14), ("a1", 15), ("e0", 20), ("e1", 21), ("j", 30), ("l", 31),
        ("lj", 32), ("m", 33), ("o0", 40), ("o1", 41), ("r", 45), ("v", 50), ("z", 55), ("k", 60), ("t", 61), ("i0", 62), ("i1", 63), ("sj", 64)];
    // замок with the stress on its first vowel (a mark), ёлка with ё (marks), and an abbreviation spelled out in full.
    let mut words = vec![("замок", vec![1, 0x40]), ("елка", vec![1, 0x80 | 0x40]), ("рлк", vec![0xFF, 4, 8, 16, 8, 12])];
    // Enough other words for several blocks, so the search crosses them.
    let filler: Vec<String> = (0..300).map(|i| format!("слово{:03}", i)).collect();
    words.extend(filler.iter().map(|w| (w.as_str(), vec![0u8])));
    let file = build(&table, &words);
    let d = russian::Dictionary::parse(&file, true).unwrap();
    assert_eq!(d.words(), 303);
    assert_eq!(d.lookup("замок").unwrap(), names(&["z", "a1", "m", "o0", "k"]));
    assert_eq!(d.lookup("елка").unwrap(), names(&["j", "o1", "l", "k", "a0"]));
    assert_eq!(d.lookup("рлк").unwrap(), names(&["e0", "r", "e0", "lj"]));
    assert_eq!(d.lookup("слово250").unwrap(), russian::rules("слово250"));
    assert!(d.lookup("замки").is_none() && d.lookup("яяя").is_none() && d.lookup("aaa").is_none());
    // Text: lower case, punctuation and spaces as phonemes, the blank between; "^" and "$" around.
    assert_eq!(d.ids("Замок, елка."), vec![1, 0, 55, 0, 15, 0, 33, 0, 40, 0, 60, 0, 8, 0, 3, 0, 30, 0, 41, 0, 31, 0, 60, 0, 14, 0, 10, 0, 2]);
    let mut damaged = file.clone();
    damaged[40] ^= 1;
    assert!(russian::Dictionary::parse(&damaged, true).is_err());
}

#[test]
fn sentences_as_vosk_tts() {
    // By hand: MIND_TTS_DICTIONARY (dictionary.py's file for tts-ru-vosk-0.7) and MIND_TTS_IDS (vosk_ids.py's).
    let (Ok(path), Ok(ids)) = (std::env::var("MIND_TTS_DICTIONARY"), std::env::var("MIND_TTS_IDS")) else { return };
    let file = std::fs::read(path).unwrap();
    let start = std::time::Instant::now();
    let d = russian::Dictionary::parse(&file, true).unwrap();
    println!("{} words, read and checked in {:.2} s", d.words(), start.elapsed().as_secs_f32());
    let mut count = 0;
    for line in std::fs::read_to_string(ids).unwrap().lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        let want: Vec<i64> = parts[1].split(' ').map(|v| v.parse().unwrap()).collect();
        let got = d.ids(parts[2]);
        assert_eq!(got, want, "{}: {}", parts[0], parts[2]);
        count += 1;
    }
    println!("{} sentences, the same ids as vosk-tts", count);
}
