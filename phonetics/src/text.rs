// Text -> phonemes: Russian orthography (nearly phonetic) and simplified reading rules for Latin script.
use crate::phonemes::{Ph, Unit};

pub const WORD_GAP_MS: u16 = 100;
const COMMA_MS: u16 = 220;

pub struct Out<'a> { pub units: &'a mut [Unit], pub len: usize }

impl Out<'_> {
    fn push(&mut self, ph: Ph) { if self.len < self.units.len() { self.units[self.len] = Unit { ph, soft: false, stress: false }; self.len += 1; } }
    fn last(&mut self) -> Option<&mut Unit> { self.len.checked_sub(1).map(|i| &mut self.units[i]) }
    // Palatalize the preceding consonant (ж, ш, ц are always hard; ч, щ, й are always soft).
    fn soften(&mut self) { if let Some(u) = self.last() { if !u.ph.vowel() && !matches!(u.ph, Ph::Zh | Ph::Sh | Ph::Ts | Ph::Pause(_) | Ph::End(_)) { u.soft = true; } } }
    fn ends_with_consonant(&self, start: usize) -> bool { self.len > start && !self.units[self.len - 1].ph.vowel() && self.units[self.len - 1].ph != Ph::J }
}

// Stress dictionary for forms where the heuristic is wrong: one word form per line, stressed vowel uppercase.
const STRESS: &str = include_str!("../data/stress_ru.txt");
const RU_VOWELS: &str = "аеёиоуыэюя";

fn fold(c: char) -> char { match c.to_lowercase().next().unwrap_or(c) { 'ё' => 'е', l => l } }

/// Binary search for a line in a sorted dictionary (the first line is a header); `order` compares a line with the target.
fn sorted_lookup(table: &'static str, order: impl Fn(&str) -> core::cmp::Ordering) -> Option<&'static str> {
    let body = &table[table.find('\n')? + 1..];
    let (mut lo, mut hi) = (0, body.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        let start = body.as_bytes()[..mid].iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1);
        let end = body[start..].find('\n').map_or(body.len(), |p| start + p);
        match order(&body[start..end]) {
            core::cmp::Ordering::Less => lo = end + 1,
            core::cmp::Ordering::Greater => hi = start,
            core::cmp::Ordering::Equal => return Some(&body[start..end]),
        }
    }
    None
}

/// Word form from the stress dictionary (lines sorted by spelling with "ё" folded to "е"); "е" in the text matches "ё".
fn dictionary_entry(word: &[char]) -> Option<&'static str> {
    let line = sorted_lookup(STRESS, |line| line.chars().map(fold).cmp(word.iter().copied().map(fold)))?;
    // A "ё" typed in the text must also be in the dictionary (otherwise it is a different word: "осёл" is not "осел").
    line.chars().zip(word).all(|(l, &c)| c != 'ё' || matches!(l, 'ё' | 'Ё')).then_some(line)
}

fn cyrillic(c: char) -> bool { ('а'..='я').contains(&c) || c == 'ё' }

const RU_DIGITS: [&str; 10] = ["ноль", "один", "два", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять"];
const EN_DIGITS: [&str; 10] = ["ziro", "wan", "tu", "thri", "for", "faiv", "siks", "seven", "eit", "nain"];

fn russian(word: &[char], out: &mut Out) {
    let start = out.len;
    let mut chars = [' '; 64]; let mut n = word.len().min(64); chars[..n].copy_from_slice(&word[..n]);
    // The dictionary gives the stressed vowel and restores "ё" when the text was typed with "е".
    let entry = dictionary_entry(word);
    let known = entry.and_then(|line| { let upper = line.chars().position(char::is_uppercase)?; Some(line.chars().take(upper).filter(|c| RU_VOWELS.contains(c.to_lowercase().next().unwrap_or(*c))).count()) });
    if let Some(line) = entry { for (c, l) in chars[..n].iter_mut().zip(line.chars()) { *c = l.to_lowercase().next().unwrap_or(l); } }
    // "-тся/-ться" reads as [ца]; "-ого/-его" as [ово/ево] (except "ого", "много", "строго", "дорого").
    let ends = |chars: &[char], tail: &str| chars.len() >= tail.chars().count() && chars[chars.len() - tail.chars().count()..].iter().copied().eq(tail.chars());
    if ends(&chars[..n], "ться") { n -= 4; chars[n] = 'ц'; chars[n + 1] = 'а'; n += 2; }
    else if ends(&chars[..n], "тся") { n -= 3; chars[n] = 'ц'; chars[n + 1] = 'а'; n += 2; }
    if n >= 3 && (ends(&chars[..n], "ого") || ends(&chars[..n], "его")) && !["ого", "много", "строго", "дорого"].iter().any(|w| w.chars().eq(chars[..n].iter().copied())) { chars[n - 2] = 'в'; }
    for &c in chars[..n].iter() {
        let after_consonant = out.ends_with_consonant(start);
        let iotated = |out: &mut Out, v: Ph| { if after_consonant { out.soften(); } else { out.push(Ph::J); } out.push(v); };
        match c {
            'а' => out.push(Ph::A), 'у' => out.push(Ph::U), 'ы' => out.push(Ph::Y), 'э' => out.push(Ph::E),
            'о' => out.push(Ph::O),
            'и' => { if after_consonant && matches!(out.units[out.len - 1].ph, Ph::Zh | Ph::Sh | Ph::Ts) { out.push(Ph::Y) } else { out.soften(); out.push(Ph::I) } }
            'е' => iotated(out, Ph::E), 'ё' => iotated(out, Ph::O), 'ю' => iotated(out, Ph::U), 'я' => iotated(out, Ph::A),
            'ь' => out.soften(),
            'ъ' => out.push(Ph::Pause(0)),
            'б' => out.push(Ph::B), 'в' => out.push(Ph::V), 'г' => out.push(Ph::G), 'д' => out.push(Ph::D), 'ж' => out.push(Ph::Zh),
            'з' => out.push(Ph::Z), 'й' => out.push(Ph::J), 'к' => out.push(Ph::K), 'л' => out.push(Ph::L), 'м' => out.push(Ph::M),
            'н' => out.push(Ph::N), 'п' => out.push(Ph::P), 'р' => out.push(Ph::R), 'с' => out.push(Ph::S), 'т' => out.push(Ph::T),
            'ф' => out.push(Ph::F), 'х' => out.push(Ph::X), 'ц' => out.push(Ph::Ts), 'ш' => out.push(Ph::Sh),
            'ч' => { out.push(Ph::Ch); out.soften(); } 'щ' => { out.push(Ph::Shch); out.soften(); }
            _ => {}
        }
    }
    // Double consonants are one sound; word-final devoicing and voicing assimilation before an obstruent.
    let mut i = start + 1;
    while i < out.len { if out.units[i].ph == out.units[i - 1].ph && !out.units[i].ph.vowel() { out.units.copy_within(i + 1..out.len, i); out.len -= 1; } else { i += 1; } }
    if out.len > start { let last = &mut out.units[out.len - 1]; last.ph = last.ph.devoiced(); }
    // Heuristic stress (no dictionary entry): "ё" is stressed; word ending in a consonant -> last syllable, in a vowel -> penultimate.
    let yo = chars[..n].iter().position(|&c| c == 'ё').map(|p| chars[..p].iter().filter(|c| "аоуыэиеёюя".contains(**c)).count());
    stress(out, start, known.or(yo));
    // Akanye and ikanye: unstressed [о] sounds as [а], unstressed [е], [а] after a soft consonant as [и] (except word-finally).
    for i in start..out.len {
        let unit = out.units[i];
        if !unit.ph.vowel() || unit.stress { continue; }
        let soft_before = i > start && (out.units[i - 1].soft || out.units[i - 1].ph == Ph::J);
        out.units[i].ph = match unit.ph { Ph::O => Ph::A, Ph::E | Ph::A if soft_before && i + 1 < out.len => Ph::I, other => other };
    }
    for i in (start..out.len.saturating_sub(1)).rev() {
        let (current, next) = (out.units[i].ph, out.units[i + 1].ph);
        if !current.obstruent() || !next.obstruent() || next == Ph::V { continue; }
        out.units[i].ph = if next.voiceless() { current.devoiced() } else { current.voiced() };
    }
}

// Marks the stressed vowel of a word: explicit index, or the rule "final consonant -> last syllable, otherwise penultimate".
fn stress(out: &mut Out, start: usize, explicit: Option<usize>) {
    let vowels = (start..out.len).filter(|&i| out.units[i].ph.vowel()).count();
    if vowels == 0 { return; }
    let ends_with_vowel = out.units[out.len - 1].ph.vowel();
    let target = explicit.unwrap_or(if vowels == 1 || !ends_with_vowel { vowels - 1 } else { vowels - 2 });
    if let Some(i) = (start..out.len).filter(|&i| out.units[i].ph.vowel()).nth(target.min(vowels - 1)) { out.units[i].stress = true; }
}

// Manual overrides for the CMUdict lexicon (checked first). Notation: a o u e i y = vowels, @ = schwa, & = [æ], ' = stress,
// S Z C T D = [ʃ ʒ tʃ θ ð], I U A R = [ɪ ʊ ʌ ɝ], other letters = the same-named consonants (j = [j], x = [x], h = aspiration).
const LEXICON: &[(&str, &str)] = &[
    ("the", "T@"), ("a", "@"), ("an", "&n"), ("is", "'iz"), ("are", "'ar"), ("was", "w'@z"), ("you", "j'u"), ("to", "t'u"), ("of", "'@v"),
    ("i", "'aj"), ("my", "m'aj"), ("your", "j'or"), ("what", "w'@t"), ("one", "w'@n"), ("two", "t'u"), ("three", "Tr'i"), ("four", "f'or"),
    ("five", "f'ajv"), ("six", "s'iks"), ("seven", "s'ev@n"), ("eight", "'ejt"), ("nine", "n'ajn"), ("ten", "t'en"), ("zero", "z'iro"),
    ("hello", "h@l'ou"), ("world", "w'@rld"), ("ready", "r'edi"), ("system", "s'ist@m"), ("thank", "T'&nk"), ("thanks", "T'&nks"),
    ("very", "v'eri"), ("much", "m'@C"), ("good", "g'ud"), ("morning", "m'ornin"), ("evening", "'ivnin"), ("night", "n'ajt"),
    ("open", "'oup@n"), ("door", "d'or"), ("this", "T'is"), ("that", "T'&t"), ("yes", "j'es"), ("no", "n'ou"), ("ok", "ouk'ej"),
    ("please", "pl'iz"), ("error", "'er@r"), ("done", "d'@n"), ("one's", "w'@nz"), ("computer", "k@mpj'ut@r"), ("mind", "m'ajnd"),
    ("ship", "S'ip"), ("sound", "s'aund"), ("music", "mj'uzik"), ("welcome", "w'elk@m"), ("there", "T'er"), ("where", "w'er"),
    ("here", "h'ir"), ("we", "w'i"), ("he", "h'i"), ("she", "S'i"), ("they", "T'ej"), ("be", "b'i"), ("do", "d'u"), ("have", "h'&v"),
];

// Pronunciations of frequent words from CMUdict: one "word code" line each, sorted alphabetically.
const LEXICON_EN: &str = include_str!("../data/lexicon_en.txt");

fn lexicon(word: &[char], out: &mut Out) -> bool {
    let code = match LEXICON.iter().find(|(w, _)| w.chars().eq(word.iter().copied())) {
        Some((_, code)) => *code,
        None => match sorted_lookup(LEXICON_EN, |line| line.split(' ').next().unwrap_or("").chars().cmp(word.iter().copied())) {
            Some(line) => line.split(' ').nth(1).unwrap_or(""),
            None => return false,
        },
    };
    let mut stress = false;
    for c in code.chars() {
        let ph = match c {
            '\'' => { stress = true; continue; }
            'a' => Ph::A, 'o' => Ph::O, 'u' => Ph::U, 'e' => Ph::E, 'i' => Ph::I, 'y' => Ph::Y, '@' => Ph::Schwa, '&' => Ph::Ae, 'I' => Ph::Ih, 'U' => Ph::Uh, 'A' => Ph::Ah, 'R' => Ph::Er, 'D' => Ph::Dh,
            'j' => Ph::J, 'w' => Ph::W, 'l' => Ph::L, 'r' => Ph::R, 'm' => Ph::M, 'n' => Ph::N, 'p' => Ph::P, 'b' => Ph::B, 't' => Ph::T,
            'd' => Ph::D, 'k' => Ph::K, 'g' => Ph::G, 'f' => Ph::F, 'v' => Ph::V, 's' => Ph::S, 'z' => Ph::Z, 'S' => Ph::Sh, 'Z' => Ph::Zh,
            'C' => Ph::Ch, 'T' => Ph::Th, 'x' => Ph::X, 'h' => Ph::H, _ => continue,
        };
        out.push(ph);
        if stress && ph.vowel() { out.last().unwrap().stress = true; stress = false; }
    }
    true
}

fn latin(word: &[char], out: &mut Out) {
    if lexicon(word, out) { return; }
    let start = out.len;
    let n = word.len();
    let at = |i: usize| if i < n { word[i] } else { ' ' };
    let vowel = |c: char| "aeiouy".contains(c);
    // Silent final "e" and "magic e": name -> [neim], time -> [taim].
    let silent_e = n > 2 && at(n - 1) == 'e' && !vowel(at(n - 2)) && word[..n - 1].iter().any(|&c| vowel(c));
    let magic = silent_e && n >= 3 && vowel(at(n - 3)) && (n < 4 || !vowel(at(n - 4)));
    let end = if silent_e { n - 1 } else { n };
    let mut i = 0;
    while i < end {
        let (c, next) = (at(i), at(i + 1));
        let pair = |a: char, b: char| c == a && next == b;
        let mut step = 1;
        match c {
            _ if pair('s', 'h') => { out.push(Ph::Sh); step = 2; }
            _ if pair('c', 'h') || (c == 't' && next == 'c' && at(i + 2) == 'h') => { out.push(Ph::Ch); step = if c == 't' { 3 } else { 2 }; }
            _ if pair('t', 'h') => { out.push(Ph::Th); step = 2; }
            _ if pair('p', 'h') => { out.push(Ph::F); step = 2; }
            _ if pair('c', 'k') => { out.push(Ph::K); step = 2; }
            _ if pair('q', 'u') => { out.push(Ph::K); out.push(Ph::W); step = 2; }
            _ if pair('w', 'h') => { out.push(Ph::W); step = 2; }
            _ if pair('n', 'g') => { out.push(Ph::N); step = 2; }
            _ if pair('g', 'h') => { step = 2; }
            _ if pair('k', 'n') && i == 0 => { out.push(Ph::N); step = 2; }
            _ if pair('e', 'e') || pair('e', 'a') || pair('i', 'e') => { out.push(Ph::I); out.last().unwrap().stress = true; step = 2; }
            _ if pair('o', 'o') => { out.push(Ph::U); step = 2; }
            _ if pair('o', 'u') || pair('o', 'w') => { out.push(Ph::A); out.push(Ph::U); step = 2; }
            _ if pair('a', 'i') || pair('a', 'y') || pair('e', 'i') || pair('e', 'y') => { out.push(Ph::E); out.push(Ph::J); step = 2; }
            _ if pair('o', 'a') => { out.push(Ph::O); out.push(Ph::U); step = 2; }
            'a' if magic && i == n - 3 => { out.push(Ph::E); out.push(Ph::J); }
            'i' if (magic && i == n - 3) || (next == 'g' && at(i + 2) == 'h') => { out.push(Ph::A); out.push(Ph::J); }
            'o' if magic && i == n - 3 => { out.push(Ph::O); out.push(Ph::U); }
            'u' if magic && i == n - 3 => { out.push(Ph::J); out.push(Ph::U); }
            'a' => out.push(if next == 'r' || next == 'l' && at(i + 2) == 'l' { Ph::A } else { Ph::Ae }),
            'e' => out.push(Ph::E), 'i' => out.push(Ph::I), 'o' => out.push(Ph::O), 'u' => out.push(Ph::Schwa),
            'y' => out.push(if i == 0 { Ph::J } else { Ph::I }),
            'b' => out.push(Ph::B), 'd' => out.push(Ph::D), 'f' => out.push(Ph::F), 'g' => out.push(Ph::G), 'h' => out.push(Ph::H),
            'j' => { out.push(Ph::D); out.push(Ph::Zh); } 'k' | 'q' => out.push(Ph::K), 'l' => out.push(Ph::L), 'm' => out.push(Ph::M),
            'n' => out.push(Ph::N), 'p' => out.push(Ph::P), 'r' => out.push(Ph::R), 't' => out.push(Ph::T), 'v' => out.push(Ph::V),
            'w' => out.push(Ph::W), 'x' => { out.push(Ph::K); out.push(Ph::S); } 'z' => out.push(Ph::Z),
            's' => out.push(if i > 0 && i + 1 < end && vowel(at(i - 1)) && vowel(next) { Ph::Z } else { Ph::S }),
            'c' => out.push(if "eiy".contains(next) { Ph::S } else { Ph::K }),
            _ => {}
        }
        // Doubled consonants are read once.
        if step == 1 && !vowel(c) && next == c { step = 2; }
        i += step;
    }
    // In English, stress usually falls on the first syllable.
    if !(start..out.len).any(|i| out.units[i].stress) { stress(out, start, Some(0)); }
}

/// Parses text into phonemes; returns their count.
pub fn parse(text: &str, units: &mut [Unit]) -> usize {
    let mut out = Out { units, len: 0 };
    let russian_text = text.chars().any(|c| cyrillic(c.to_lowercase().next().unwrap_or(c)));
    let mut word = [' '; 64]; let mut len = 0;
    let flush = |word: &[char], out: &mut Out| {
        if word.is_empty() { return; }
        if out.len > 0 && !matches!(out.units[out.len - 1].ph, Ph::Pause(_) | Ph::End(_)) { out.push(Ph::Pause(WORD_GAP_MS)); }
        if word.iter().any(|&c| cyrillic(c)) { russian(word, out) } else { latin(word, out) }
    };
    for raw in text.chars() {
        let c = raw.to_lowercase().next().unwrap_or(raw);
        if cyrillic(c) || c.is_ascii_lowercase() || c == '\'' {
            if len < word.len() && c != '\'' { word[len] = c; len += 1; }
            continue;
        }
        flush(&word[..len], &mut out); len = 0;
        if let Some(digit) = c.to_digit(10) {
            let name = if russian_text { RU_DIGITS[digit as usize] } else { EN_DIGITS[digit as usize] };
            let mut spelled = [' '; 16]; let mut k = 0; for ch in name.chars() { spelled[k] = ch; k += 1; }
            flush(&spelled[..k], &mut out);
            continue;
        }
        match c {
            ',' | ';' | ':' | '-' | '—' | '(' | ')' => { if out.len > 0 { out.push(Ph::Pause(COMMA_MS)); } }
            '.' | '!' | '\n' => { if out.len > 0 { out.push(Ph::End(b'.')); } }
            '?' => { if out.len > 0 { out.push(Ph::End(b'?')); } }
            _ => {}
        }
    }
    flush(&word[..len], &mut out);
    if out.len > 0 && !matches!(out.units[out.len - 1].ph, Ph::End(_)) { out.push(Ph::End(b'.')); }
    out.len
}

#[cfg(test)]
mod tests {
    use super::*;
    fn phonemes(text: &str) -> Vec<(Ph, bool)> {
        let mut units = [Unit { ph: Ph::Pause(0), soft: false, stress: false }; 256];
        let n = parse(text, &mut units);
        units[..n].iter().map(|u| (u.ph, u.soft)).filter(|(p, _)| !matches!(p, Ph::Pause(_) | Ph::End(_))).collect()
    }
    #[test]
    fn russian_rules_devoice_soften_and_read_tsya() {
        assert_eq!(phonemes("дуб"), [(Ph::D, false), (Ph::U, false), (Ph::P, false)]);
        assert_eq!(phonemes("шум"), [(Ph::Sh, false), (Ph::U, false), (Ph::M, false)]);
        assert_eq!(phonemes("мать"), [(Ph::M, false), (Ph::A, false), (Ph::T, true)]);
        assert_eq!(phonemes("мять"), [(Ph::M, true), (Ph::A, false), (Ph::T, true)]);
        assert!(phonemes("учится").ends_with(&[(Ph::Ts, false), (Ph::A, false)]));
        assert_eq!(&phonemes("его")[..3], [(Ph::J, false), (Ph::I, false), (Ph::V, false)]); // [йиво]: stress from the dictionary, ikanye, "г" -> [в]
        assert_eq!(phonemes("молоко")[1].0, Ph::A); // akanye in an unstressed syllable
        assert_eq!(phonemes("жи")[1].0, Ph::Y);
    }
    #[test]
    fn stress_dictionary_overrides_heuristic() {
        let stressed = |text: &str| { let mut units = [Unit { ph: Ph::Pause(0), soft: false, stress: false }; 64]; let n = parse(text, &mut units); units[..n].iter().filter(|u| u.ph.vowel()).position(|u| u.stress) };
        assert_eq!(stressed("добрый"), Some(0)); // the heuristic would pick the last syllable
        assert_eq!(stressed("тебя"), Some(1)); // and the penultimate one here
        assert_eq!(stressed("заполнена"), Some(1));
        assert_eq!(stressed("работа"), Some(1)); // not in the dictionary: the heuristic is right
        assert_eq!(phonemes("добрый")[1].0, Ph::O); // stressed "о" is not reduced
        assert_eq!(phonemes("еще")[3].0, Ph::O); // "ё" from the dictionary: [йищё]
        assert_eq!(stressed("зеленый"), Some(1));
        assert!(dictionary_entry(&"дёвушка".chars().collect::<Vec<_>>()).is_none()); // a typed "ё" does not match a dictionary "е"
        // Every dictionary entry is found by binary search, both with "ё" and spelled with "е".
        for line in STRESS.lines().filter(|l| !l.starts_with('#')) {
            let word: Vec<char> = line.chars().flat_map(char::to_lowercase).collect();
            assert_eq!(dictionary_entry(&word), Some(line));
            assert_eq!(dictionary_entry(&word.iter().map(|&c| fold(c)).collect::<Vec<_>>()), Some(line));
        }
    }
    #[test]
    fn latin_rules_and_lexicon() {
        assert_eq!(phonemes("the")[0].0, Ph::Th);
        assert_eq!(phonemes("ship"), [(Ph::Sh, false), (Ph::I, false), (Ph::P, false)]);
        assert_eq!(phonemes("2").len(), 2); // two -> [t u]
        assert_eq!(phonemes("weather")[..3], [(Ph::W, false), (Ph::E, false), (Ph::Dh, false)]); // from CMUdict, not from the rules
        for line in LEXICON_EN.lines().filter(|l| !l.starts_with('#')) {
            let word: Vec<char> = line.split(' ').next().unwrap().chars().collect();
            assert!(sorted_lookup(LEXICON_EN, |l| l.split(' ').next().unwrap().chars().cmp(word.iter().copied())) == Some(line), "{line}");
        }
    }
}
