//! What `beep` plays (issue u005): its arguments as notes — a frequency alone sounds 500 ms, a frequency and a duration
//! as long as that; 0 Hz is a pause. No system calls: tests/beep_host.rs.

pub const DEFAULT_MS: u32 = 500;
pub const MAX_NOTES: usize = 64;
pub const MAX_MS: u32 = 10_000;

/// One note: Hz (0: a pause) and milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Note { pub hz: u32, pub ms: u32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error<'a> { NotANumber(&'a str), Frequency(u32), Duration(u32), TooMany }

impl Error<'_> {
    pub fn describe(&self, out: &mut impl core::fmt::Write) -> core::fmt::Result {
        match self {
            Error::NotANumber(word) => write!(out, "{}: NOT A NUMBER", word),
            Error::Frequency(hz) => write!(out, "{} HZ: A FREQUENCY IS 20-20000 HZ, OR 0 FOR A PAUSE", hz),
            Error::Duration(ms) => write!(out, "{} MS: A DURATION IS 1-{} MS", ms, MAX_MS),
            Error::TooMany => write!(out, "AT MOST {} NOTES", MAX_NOTES),
        }
    }
}

/// The notes in `args` (numbers separated by spaces or commas): frequency, duration, frequency, duration, …; a last
/// frequency without its duration sounds `DEFAULT_MS`. Returns how many went into `notes`.
pub fn parse<'a>(args: &'a str, notes: &mut [Note; MAX_NOTES]) -> Result<usize, Error<'a>> {
    let mut count = 0;
    let mut words = args.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|w| !w.is_empty());
    while let Some(word) = words.next() {
        let hz: u32 = word.parse().map_err(|_| Error::NotANumber(word))?;
        if hz != 0 && !(20..=20_000).contains(&hz) { return Err(Error::Frequency(hz)); }
        let ms = match words.next() {
            Some(word) => word.parse::<u32>().map_err(|_| Error::NotANumber(word))?,
            None => DEFAULT_MS,
        };
        if !(1..=MAX_MS).contains(&ms) { return Err(Error::Duration(ms)); }
        if count == MAX_NOTES { return Err(Error::TooMany); }
        notes[count] = Note { hz, ms };
        count += 1;
    }
    Ok(count)
}

/// Fills `frames` stereo frames of `hz` at `rate` from `phase` (a full turn is 2^32), fading in over the note's first
/// `fade` frames and out over its last (`at` frames into a note of `total`), so notes do not click; returns the phase.
pub fn fill(out: &mut [i16], hz: u32, rate: u32, mut phase: u32, at: usize, total: usize) -> u32 {
    let step = ((hz as u64) << 32) / rate.max(1) as u64;
    let fade = (rate as usize / 500).max(1); // 2 ms
    for (i, frame) in out.chunks_exact_mut(2).enumerate() {
        let value = if hz == 0 { 0 } else {
            // A sine from two parabolas: peak 8192.
            let p = phase >> 16;
            let half = (p & 0x7FFF) as i32;
            let v = half * (32768 - half) >> 15;
            if p & 0x8000 == 0 { v } else { -v }
        };
        let n = at + i;
        let gain = fade.min(n + 1).min(total.saturating_sub(n)).min(fade) as i32;
        let sample = (value * gain / fade as i32) as i16;
        frame[0] = sample; frame[1] = sample;
        phase = phase.wrapping_add(step as u32);
    }
    phase
}
