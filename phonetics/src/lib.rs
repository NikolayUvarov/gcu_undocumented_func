#![no_std]
//! Text to phonemes for Russian and English and the phoneme set, shared by the speech synthesizer (`tts`) and the voice
//! command recognizer (`hear`, `mind::voice`): the synthesizer speaks the phonemes, the recognizer's grammar is spelled
//! in them.
pub mod phonemes;
pub mod text;
