//! Which voice takes a sentence (252-APP-0128), without system calls: tests/tts_host.rs.

/// A sentence with a Cyrillic letter is Russian, for the neural voice; any other is the formant voice's.
pub fn russian(sentence: &str) -> bool { sentence.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) }
