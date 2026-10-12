//! Russian text to a Vosk TTS voice's phoneme ids (252, step 3), as vosk-tts makes them (Synth.g2p_noembed): the text
//! in lower case, cut at spaces and punctuation; each word's pronunciation from the dictionary
//! (scripts/voice_tts/dictionary.py's MINDDIC1 file), else from vosk-tts's letter-to-sound rules (vosk_tts/g2p.py,
//! Apache-2.0, ported here); punctuation as its own phonemes; "^" first, "$" last, the blank (0) between all of them.
//! Host tests include this file.
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Words per block of the dictionary.
const BLOCK: usize = 64;

/// A MINDDIC1 dictionary in memory: its phoneme table and its blocks of words, read in place.
pub struct Dictionary<'f> {
    /// Per phoneme: its name and its id in the voice.
    phonemes: Vec<(String, u8)>,
    offsets: Vec<u32>,
    blocks: &'f [u8],
    words: usize,
}

fn fnv1a(bytes: &[u8]) -> u32 { bytes.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193)) }

fn u32_at(b: &[u8], at: usize) -> Option<u32> { b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())) }

impl<'f> Dictionary<'f> {
    /// Reads a dictionary file; `check`: verify its checksum (a pass over every byte).
    pub fn parse(file: &'f [u8], check: bool) -> Result<Self, &'static str> {
        if file.len() < 28 || &file[..8] != b"MINDDIC1" { return Err("not a dictionary"); }
        let body = &file[..file.len() - 4];
        if check && fnv1a(body) != u32_at(file, file.len() - 4).unwrap() { return Err("checksum"); }
        if u32_at(body, 8) != Some(1) { return Err("version"); }
        let (words, blocks, count) = (u32_at(body, 12).unwrap() as usize, u32_at(body, 16).unwrap() as usize, u32_at(body, 20).unwrap() as usize);
        let mut at = 24;
        let mut phonemes = Vec::with_capacity(count);
        for _ in 0..count {
            let n = *body.get(at).ok_or("cut short")? as usize;
            let name = core::str::from_utf8(body.get(at + 1..at + 1 + n).ok_or("cut short")?).map_err(|_| "a phoneme is not UTF-8")?;
            let id = *body.get(at + 1 + n).ok_or("cut short")?;
            phonemes.push((name.to_string(), id));
            at += n + 2;
        }
        let offsets: Vec<u32> = (0..blocks).map(|k| u32_at(body, at + 4 * k).ok_or("cut short")).collect::<Result<_, _>>()?;
        let data = &body[at + 4 * blocks..];
        if offsets.windows(2).any(|w| w[0] > w[1]) || offsets.last().is_some_and(|&o| o as usize > data.len()) { return Err("block offsets"); }
        Ok(Self { phonemes, offsets, blocks: data, words })
    }

    pub fn words(&self) -> usize { self.words }

    /// The voice's id of phoneme `name`.
    pub fn id(&self, name: &str) -> Option<u8> { self.phonemes.iter().find(|p| p.0 == name).map(|p| p.1) }

    fn block(&self, k: usize) -> &'f [u8] {
        let end = self.offsets.get(k + 1).map_or(self.blocks.len(), |&o| o as usize);
        &self.blocks[self.offsets[k] as usize..end]
    }

    /// A word's pronunciation as phoneme names, if the dictionary has it.
    pub fn lookup(&self, word: &str) -> Option<Vec<String>> {
        let key = encode(word)?;
        // The last block whose first word is not after the key.
        let (mut lo, mut hi) = (0, self.offsets.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            let first = self.block(mid).get(2..2 + *self.block(mid).get(1)? as usize)?;
            if first <= &key[..] { lo = mid + 1 } else { hi = mid }
        }
        let block = self.block(lo.checked_sub(1)?);
        let (mut at, mut current) = (0, Vec::with_capacity(32));
        for _ in 0..BLOCK {
            if at >= block.len() { break; }
            let (shared, rest) = (block[at] as usize, block[at + 1] as usize);
            current.truncate(shared);
            current.extend_from_slice(block.get(at + 2..at + 2 + rest)?);
            at += 2 + rest;
            let n = *block.get(at)?;
            let tail = if n == 0xFF { let count = *block.get(at + 1)? as usize; at += 2 + count; &block[at - count..at] } else { at += 1 + n as usize; block.get(at - n as usize..at)? };
            if current[..] == key[..] {
                return Some(if n == 0xFF {
                    tail.iter().filter_map(|&i| self.phonemes.get(i as usize).map(|p| p.0.clone())).collect()
                } else {
                    rules(&marked(word, tail))
                });
            }
            if current[..] > key[..] { return None; }
        }
        None
    }

    /// The phoneme ids of `text` for the voice, as vosk-tts makes them.
    pub fn ids(&self, text: &str) -> Vec<i64> {
        let text = text.trim().replace('—', "-").to_lowercase();
        let mut names: Vec<String> = Vec::new();
        let mut word = String::new();
        let flush = |word: &mut String, names: &mut Vec<String>| {
            if word.is_empty() { return; }
            if word == "-" { names.push(word.clone()); } else { names.extend(self.lookup(word).unwrap_or_else(|| rules(word))); }
            word.clear();
        };
        for c in text.chars() {
            if PUNCTUATION.contains(c) { flush(&mut word, &mut names); names.push(c.to_string()); } else { word.push(c); }
        }
        flush(&mut word, &mut names);
        let mut ids = alloc::vec![self.id("^").unwrap_or(1) as i64];
        for name in names.iter().map(|n| n.as_str()).chain(core::iter::once("$")) {
            // A phoneme the voice does not know (a digit, a Latin letter) is left out; vosk-tts would fail on it.
            if let Some(id) = self.id(name) { ids.push(0); ids.push(id as i64); }
        }
        ids
    }
}

/// Characters that end a word and are phonemes of their own (vosk-tts's pattern).
const PUNCTUATION: &str = ",.?!;:\"() ";

// A word in Windows-1251, as the dictionary keeps its words; None if it has another character.
fn encode(word: &str) -> Option<Vec<u8>> {
    word.chars().map(|c| match c {
        '\u{0}'..='\u{7F}' => Some(c as u8),
        'а'..='я' => Some((c as u32 - 'а' as u32 + 0xE0) as u8),
        'ё' => Some(0xB8),
        _ => None,
    }).collect()
}

const VOWEL_LETTERS: &str = "аяуюоёэеиы";

// The word with its marks applied: "+" before each stressed vowel, ё for each е marked so.
fn marked(word: &str, marks: &[u8]) -> String {
    let mut out = String::new();
    let mut vowel = 0u8;
    for c in word.chars() {
        if VOWEL_LETTERS.contains(c) {
            let mark = marks.iter().find(|&&m| m & 0x3F == vowel).copied().unwrap_or(0);
            if mark & 0x40 != 0 { out.push('+'); }
            out.push(if mark & 0x80 != 0 && c == 'е' { 'ё' } else { c });
            vowel += 1;
        } else {
            out.push(c);
        }
    }
    out
}

/// vosk-tts's letter-to-sound rules: a stress mark "+" goes before a vowel; consonants are soft before я ё ю и ь е;
/// я ю е ё after a vowel, a sign, a hyphen or the start add a "j"; vowels carry 1 when stressed, else 0.
pub fn rules(word: &str) -> Vec<String> {
    let mut phones: Vec<(char, bool)> = Vec::new();
    let mut stress = false;
    for c in core::iter::once('#').chain(word.chars()).chain(core::iter::once('#')) {
        if c == '+' { stress = true; } else { phones.push((c, stress)); stress = false; }
    }
    let soft = |c: char| "яёюиье".contains(c);
    let mut out: Vec<String> = Vec::new();
    let mut prev = String::new();
    for (i, &(c, stressed)) in phones.iter().enumerate() {
        let last = i + 1 == phones.len();
        let name: String = match c {
            _ if !last && hard(c).is_some() => { let mut n = String::from(hard(c).unwrap()); if soft(phones[i + 1].0) { n.push('j'); } n }
            _ if !last && other(c).is_some() => String::from(other(c).unwrap()),
            _ => c.to_string(),
        };
        if prev.chars().count() == 1 && "#ъьаяоёуюэеиы-".contains(prev.as_str()) && "яюеё".contains(c) { out.push(String::from("j")); }
        out.push(match vowel(c) { Some(v) => alloc::format!("{}{}", v, if stressed { 1 } else { 0 }), None => name.clone() });
        prev = name;
    }
    out.retain(|p| !matches!(p.as_str(), "#" | "+" | "-" | "ь" | "ъ"));
    out
}

fn hard(c: char) -> Option<&'static str> {
    Some(match c { 'б' => "b", 'в' => "v", 'г' | 'Г' => "g", 'д' => "d", 'з' => "z", 'к' => "k", 'л' => "l", 'м' => "m", 'н' => "n", 'п' => "p", 'р' => "r", 'с' => "s", 'т' => "t", 'ф' => "f", 'х' => "h", _ => return None })
}

fn other(c: char) -> Option<&'static str> {
    Some(match c { 'ж' => "zh", 'ц' => "c", 'ч' => "ch", 'ш' => "sh", 'щ' => "sch", 'й' => "j", _ => return None })
}

fn vowel(c: char) -> Option<&'static str> {
    Some(match c { 'а' | 'я' => "a", 'у' | 'ю' => "u", 'о' | 'ё' => "o", 'э' | 'е' => "e", 'и' => "i", 'ы' => "y", _ => return None })
}
