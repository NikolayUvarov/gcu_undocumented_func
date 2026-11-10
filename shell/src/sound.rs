//! The shell's `volume` and `mic` (000-APP-0076, 000-APP-0077, `audio.wit` 1.2): their words, the lines they print, and
//! `data/sound.conf`, which keeps the levels across boots. No system calls: tests/shell_host.rs.

use alloc::format;
use alloc::string::String;
use core::fmt::Write;

/// The levels the shell sets again at its start.
pub const FILE: &str = "data/sound.conf";

/// What `volume` asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Volume { Show, Set(u8), Mute(bool) }

/// What `mic` asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mic { Show, Set(u8), Boost(bool) }

pub const VOLUME_USAGE: &str = "VOLUME [0-100 | MUTE | UNMUTE]: THE OUTPUT'S LEVEL IN PER CENT OF ITS RANGE";
pub const MIC_USAGE: &str = "MIC [0-100 | BOOST ON | BOOST OFF]: THE MICROPHONE'S GAIN IN PER CENT OF ITS RANGE";

// A level in per cent: 0 to 100, digits alone.
fn percent(word: &str) -> Option<u8> {
    (!word.is_empty() && word.len() <= 3 && word.bytes().all(|b| b.is_ascii_digit())).then(|| word.parse::<u8>().ok()).flatten().filter(|&p| p <= 100)
}

pub fn volume(args: &str) -> Result<Volume, &'static str> {
    let words: alloc::vec::Vec<&str> = args.split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(Volume::Show),
        [word] if word.eq_ignore_ascii_case("mute") => Ok(Volume::Mute(true)),
        [word] if word.eq_ignore_ascii_case("unmute") => Ok(Volume::Mute(false)),
        [word] => percent(word).map(Volume::Set).ok_or(VOLUME_USAGE),
        _ => Err(VOLUME_USAGE),
    }
}

pub fn mic(args: &str) -> Result<Mic, &'static str> {
    let words: alloc::vec::Vec<&str> = args.split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(Mic::Show),
        [word, state] if word.eq_ignore_ascii_case("boost") && state.eq_ignore_ascii_case("on") => Ok(Mic::Boost(true)),
        [word, state] if word.eq_ignore_ascii_case("boost") && state.eq_ignore_ascii_case("off") => Ok(Mic::Boost(false)),
        [word] => percent(word).map(Mic::Set).ok_or(MIC_USAGE),
        _ => Err(MIC_USAGE),
    }
}

/// An amplifier as `audio.wit` describes it: settings, the quarter dB between two, the one at 0 dB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amp { pub steps: u8, pub step_qdb: u8, pub zero: u8 }

/// The level a per cent stands for, in quarter dB: the nearest setting, as `audio_gw` takes it (audio_gw/src/levels.rs).
pub fn qdb(percent: u8, amp: Amp) -> i32 {
    let top = amp.steps.saturating_sub(1) as u32;
    let setting = ((percent.min(100) as u32 * top + 50) / 100) as i32;
    (setting - amp.zero as i32) * amp.step_qdb as i32
}

/// Quarter dB as `-12.25 DB`, `+6.00 DB` or `0.00 DB`.
pub fn db(qdb: i32) -> String {
    let sign = if qdb < 0 { "-" } else if qdb > 0 { "+" } else { "" };
    let q = qdb.unsigned_abs();
    format!("{}{}.{:02} DB", sign, q / 4, q % 4 * 25)
}

/// `volume`'s line: the level, its dB, and whether it is muted; the device's own amplifier or the gateway's scaling.
pub fn volume_line(percent: u8, muted: bool, amp: Amp, scaled: bool) -> String {
    format!("VOLUME {} % ({} OF {} SETTINGS{}), {}", percent, db(qdb(percent, amp)), amp.steps, if scaled { ", SCALED BY THE GATEWAY: NO AMPLIFIER" } else { "" },
        if muted { "MUTED" } else { "NOT MUTED" })
}

/// `mic`'s line: the gain, its dB, and the boost where the device has one.
pub fn mic_line(percent: u8, boost: bool, boost_qdb: u16, amp: Amp, scaled: bool) -> String {
    let mut line = format!("MIC {} % ({} OF {} SETTINGS{}), ", percent, db(qdb(percent, amp)), amp.steps, if scaled { ", SCALED BY THE GATEWAY: NO AMPLIFIER" } else { "" });
    let _ = if boost_qdb == 0 { write!(line, "NO BOOST") } else { write!(line, "BOOST {} ({})", if boost { "ON" } else { "OFF" }, db(boost_qdb as i32)) };
    line
}

/// The levels kept in `data/sound.conf`; a line it does not hold is left as the device has it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kept { pub volume: Option<u8>, pub muted: Option<bool>, pub gain: Option<u8>, pub boost: Option<bool> }

/// The file's lines `volume N`, `muted yes|no`, `gain N`, `boost on|off`; `#` lines, empty lines and any line it cannot
/// read are skipped, so a damaged file sets what it still says.
pub fn parse(text: &str) -> Kept {
    let mut kept = Kept::default();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut words = line.split_whitespace();
        let (Some(key), Some(value), None) = (words.next(), words.next(), words.next()) else { continue };
        let flag = |on: &str, off: &str| if value.eq_ignore_ascii_case(on) { Some(true) } else if value.eq_ignore_ascii_case(off) { Some(false) } else { None };
        match key {
            "volume" => if let Some(p) = percent(value) { kept.volume = Some(p) },
            "muted" => if let Some(m) = flag("yes", "no") { kept.muted = Some(m) },
            "gain" => if let Some(p) = percent(value) { kept.gain = Some(p) },
            "boost" => if let Some(b) = flag("on", "off") { kept.boost = Some(b) },
            _ => {}
        }
    }
    kept
}

/// The file's text for these levels.
pub fn render(kept: &Kept) -> String {
    let mut text = String::from("# The levels the shell sets at its start (volume, mic: 000-APP-0076, 000-APP-0077).\n");
    if let Some(v) = kept.volume { let _ = writeln!(text, "volume {}", v); }
    if let Some(m) = kept.muted { let _ = writeln!(text, "muted {}", if m { "yes" } else { "no" }); }
    if let Some(g) = kept.gain { let _ = writeln!(text, "gain {}", g); }
    if let Some(b) = kept.boost { let _ = writeln!(text, "boost {}", if b { "on" } else { "off" }); }
    text
}
