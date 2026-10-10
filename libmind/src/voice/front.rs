//! The voice front end without devices (host tests include this file): WAV files, conversion to 16 kHz mono and
//! speech detection. `Source` is anything that yields interleaved 16-bit samples; `Stream` turns a source into 16 kHz
//! mono; `Detector` cuts that stream into utterances.
use alloc::vec;
use alloc::vec::Vec;

/// The rate of the stream the recognizer works on.
pub const RATE: u32 = 16_000;

/// Interleaved 16-bit samples from a microphone, a file or a test.
pub trait Source {
    type Error;
    /// Samples per second of one channel.
    fn rate(&self) -> u32;
    /// Channels in the interleaved samples.
    fn channels(&self) -> usize;
    /// Copies the next samples (whole frames) into `out` and returns their number; 0 when none are ready yet or the
    /// source has ended (`finished`).
    fn read(&mut self, out: &mut [i16]) -> Result<usize, Self::Error>;
    /// No more samples will come.
    fn finished(&self) -> bool;
}

// ---- WAV ----

/// Why a file is not a WAV `Wav` can read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavError { NotWav, NotPcm16, BadFormat, NoData }

/// A 16-bit PCM WAV file in memory (mono, stereo or up to 8 channels, 4–192 kHz).
pub struct Wav { bytes: Vec<u8>, data: usize, len: usize, rate: u32, channels: usize, position: usize }

fn u16_at(bytes: &[u8], at: usize) -> u16 { u16::from_le_bytes([bytes[at], bytes[at + 1]]) }
fn u32_at(bytes: &[u8], at: usize) -> u32 { u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) }

impl Wav {
    /// Reads the RIFF chunks: `fmt ` (PCM or WAVE_FORMAT_EXTENSIBLE with PCM, 16 bits) before `data`; other chunks
    /// are skipped. A `data` size past the end of the file (streamed WAV) is cut to the file.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, WavError> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" { return Err(WavError::NotWav); }
        let (mut at, mut format) = (12usize, None);
        while at + 8 <= bytes.len() {
            let (id, size) = (&bytes[at..at + 4], u32_at(&bytes, at + 4) as usize);
            let body = at + 8;
            match id {
                b"fmt " => {
                    if size < 16 || body + 16 > bytes.len() { return Err(WavError::BadFormat); }
                    let tag = u16_at(&bytes, body);
                    let pcm = tag == 1 || (tag == 0xFFFE && size >= 40 && body + 26 <= bytes.len() && u16_at(&bytes, body + 24) == 1);
                    let (channels, rate, align, bits) = (u16_at(&bytes, body + 2) as usize, u32_at(&bytes, body + 4), u16_at(&bytes, body + 12) as usize, u16_at(&bytes, body + 14));
                    if !pcm || bits != 16 { return Err(WavError::NotPcm16); }
                    if !(1..=8).contains(&channels) || !(4_000..=192_000).contains(&rate) || align != channels * 2 { return Err(WavError::BadFormat); }
                    format = Some((rate, channels));
                }
                b"data" => {
                    let (rate, channels) = format.ok_or(WavError::BadFormat)?;
                    let len = size.min(bytes.len() - body) / (channels * 2) * (channels * 2);
                    return Ok(Self { bytes, data: body, len, rate, channels, position: 0 });
                }
                _ => {}
            }
            at = body.saturating_add(size).saturating_add(size & 1);
        }
        Err(if format.is_some() { WavError::NoData } else { WavError::BadFormat })
    }

    /// Frames (one sample of every channel) in the file.
    pub fn frames(&self) -> usize { self.len / (self.channels * 2) }

    pub fn duration_ms(&self) -> u64 { self.frames() as u64 * 1000 / self.rate as u64 }

    /// Starts reading from the beginning again.
    pub fn rewind(&mut self) { self.position = 0; }
}

impl Source for Wav {
    type Error = core::convert::Infallible;
    fn rate(&self) -> u32 { self.rate }
    fn channels(&self) -> usize { self.channels }
    fn read(&mut self, out: &mut [i16]) -> Result<usize, Self::Error> {
        let count = (out.len() / self.channels * self.channels).min((self.len - self.position) / 2);
        let from = &self.bytes[self.data + self.position..];
        for (i, sample) in out[..count].iter_mut().enumerate() { *sample = i16::from_le_bytes([from[i * 2], from[i * 2 + 1]]); }
        self.position += count * 2;
        Ok(count)
    }
    fn finished(&self) -> bool { self.position >= self.len }
}

// ---- Conversion to 16 kHz mono ----

// Soft-float helpers for the filter design (once per stream; `core` has no sin, sqrt).
const PI: f64 = core::f64::consts::PI;

pub(crate) fn sin(x: f64) -> f64 {
    let turns = (x / (2.0 * PI)) as i64; // no `%` for floats without libm
    let mut x = x - turns as f64 * 2.0 * PI;
    if x > PI { x -= 2.0 * PI } else if x < -PI { x += 2.0 * PI }
    if x > PI / 2.0 { x = PI - x } else if x < -PI / 2.0 { x = -PI - x }
    let x2 = x * x;
    let (mut term, mut sum) = (x, x);
    for n in 1..12 { term = -term * x2 / ((2 * n) as f64 * (2 * n + 1) as f64); sum += term; }
    sum
}

fn sqrt(x: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    let mut y = if x > 1.0 { x } else { 1.0 };
    for _ in 0..60 { let next = 0.5 * (y + x / y); if next >= y { break; } y = next; }
    y
}

// Modified Bessel function of the first kind, order 0 (for the Kaiser window).
fn bessel_i0(x: f64) -> f64 {
    let (mut term, mut sum, half) = (1.0, 1.0, x / 2.0);
    for k in 1..40 { term *= half / k as f64; let square = term * term; sum += square; if square < sum * 1e-12 { break; } }
    sum
}

fn gcd(a: u32, b: u32) -> u32 { if b == 0 { a } else { gcd(b, a % b) } }

/// Kaiser window shape: about 70 dB stopband attenuation.
const BETA: f64 = 6.75;
/// Coefficients in Q16.
const ONE: i64 = 1 << 16;

/// Converts interleaved samples at any rate to 16 kHz mono: channels are averaged, the rate is changed by L/M with a
/// polyphase windowed-sinc low-pass filter cut at 7.5 kHz (or just below the input's Nyquist frequency when the input
/// is slower), so nothing above 8 kHz folds back into the band. 16 kHz input passes through unfiltered.
pub struct Resampler {
    channels: usize,
    up: usize,
    down: usize,
    taps: usize,
    /// Phase p: taps `[p*taps, (p+1)*taps)`, oldest input first.
    coefficients: Vec<i32>,
    /// The last `taps` mono samples, twice (`history[w]` and `history[w + taps]`), so they are always contiguous.
    history: Vec<i32>,
    write: usize,
    /// Position of the next output in the upsampled timeline, relative to the newest input sample.
    position: usize,
}

impl Resampler {
    pub fn new(rate: u32, channels: usize) -> Self { Self::between(rate, RATE, channels) }

    /// From `rate` to `to` (mono out), cut just below the lower of the two Nyquist frequencies (252: a voice's
    /// 22.05 kHz to the gateway's 48 kHz).
    pub fn between(rate: u32, to: u32, channels: usize) -> Self {
        let (rate, to) = (rate.max(1), to.max(1));
        let common = gcd(to, rate);
        let (up, down) = ((to / common) as usize, (rate / common) as usize);
        let channels = channels.max(1);
        if up == 1 && down == 1 {
            return Self { channels, up, down, taps: 1, coefficients: vec![ONE as i32], history: vec![0; 2], write: 0, position: 0 };
        }
        // Taps per phase: the transition band is about 2.2 kHz wide at every input rate.
        let taps = if rate > to { (96 * rate as usize).div_ceil(48_000).max(32) } else { 32 };
        let length = up * taps;
        let cutoff = 0.5 * 0.9375 * rate.min(to) as f64 / (rate as f64 * up as f64); // cycles per upsampled sample
        let center = (length - 1) as f64 / 2.0;
        let norm = bessel_i0(BETA);
        let mut prototype = vec![0f64; length];
        let mut total = 0.0;
        for (n, h) in prototype.iter_mut().enumerate() {
            let t = n as f64 - center;
            let sinc = if t == 0.0 { 2.0 * cutoff } else { sin(2.0 * PI * cutoff * t) / (PI * t) };
            let r = 2.0 * n as f64 / (length - 1) as f64 - 1.0;
            *h = sinc * bessel_i0(BETA * sqrt(1.0 - r * r)) / norm;
            total += *h;
        }
        // Each phase sums to about 1 (the gain of L lost to zero stuffing is restored).
        let scale = up as f64 * ONE as f64 / total;
        let mut coefficients = vec![0i32; length];
        for p in 0..up {
            for j in 0..taps {
                // Newest input pairs with h[p], the one before it with h[p + L], ...; stored oldest first.
                let value = prototype[p + (taps - 1 - j) * up] * scale;
                coefficients[p * taps + j] = if value >= 0.0 { (value + 0.5) as i32 } else { (value - 0.5) as i32 };
            }
        }
        Self { channels, up, down, taps, coefficients, history: vec![0; 2 * taps], write: 0, position: 0 }
    }

    /// The input is already at 16 kHz: samples are only downmixed, not filtered.
    pub fn passthrough(&self) -> bool { self.up == 1 && self.down == 1 }

    /// The filter in Q16, phase after phase, oldest input first in each (`tts` keeps 16 → 48 kHz as a table).
    pub fn coefficients(&self) -> &[i32] { &self.coefficients }

    /// Converts whole frames of interleaved `input` and appends the 16 kHz mono samples to `out`.
    pub fn process(&mut self, input: &[i16], out: &mut Vec<i16>) {
        let channels = self.channels as i32;
        for frame in input.chunks_exact(self.channels) {
            let mono = frame.iter().map(|&s| s as i32).sum::<i32>() / channels;
            if self.passthrough() { out.push(mono as i16); continue; }
            self.history[self.write] = mono;
            self.history[self.write + self.taps] = mono;
            self.write = (self.write + 1) % self.taps;
            // Outputs whose input index is this sample: positions 0..L-1 of its upsampled slot.
            while self.position < self.up {
                let phase = &self.coefficients[self.position * self.taps..(self.position + 1) * self.taps];
                let window = &self.history[self.write..self.write + self.taps];
                let sum: i64 = phase.iter().zip(window).map(|(&c, &x)| c as i64 * x as i64).sum();
                out.push(((sum + ONE / 2) >> 16).clamp(i16::MIN as i64, i16::MAX as i64) as i16);
                self.position += self.down;
            }
            self.position -= self.up;
        }
    }
}

/// A source converted to 16 kHz mono.
pub struct Stream<S: Source> { source: S, resampler: Resampler, raw: Vec<i16> }

impl<S: Source> Stream<S> {
    pub fn new(source: S) -> Self {
        let resampler = Resampler::new(source.rate(), source.channels());
        Self { source, resampler, raw: vec![0; 8192] }
    }
    /// Reads what the source has ready and appends it to `out` as 16 kHz mono; returns the samples added (0: nothing
    /// yet, or the source has ended, see `finished`).
    pub fn read(&mut self, out: &mut Vec<i16>) -> Result<usize, S::Error> {
        let before = out.len();
        let count = self.source.read(&mut self.raw)?;
        self.resampler.process(&self.raw[..count], out);
        Ok(out.len() - before)
    }
    pub fn finished(&self) -> bool { self.source.finished() }
    pub fn source(&self) -> &S { &self.source }
    pub fn source_mut(&mut self) -> &mut S { &mut self.source }
}

// ---- Speech detection ----

/// 25 ms analysis frames every 10 ms.
pub const FRAME: usize = 400;
pub const HOP: usize = 160;
/// Voiced frames in a row that start an utterance.
pub const START_FRAMES: usize = 3;
/// Frames below the threshold that end it (200 ms).
pub const HANGOVER_FRAMES: usize = 20;
pub const MIN_MS: u64 = 200;
pub const MAX_MS: u64 = 8_000;
/// How far the bounds of an utterance may grow over quieter frames before its first and after its last voiced frame
/// (a breathy «х», a fading vowel).
pub const EXTEND_FRAMES: u64 = 25;

/// Levels are in tenths of a decibel of the mean square of 16-bit samples: 0 is digital silence, 903 a full-scale
/// square wave (0 dBFS; a full-scale sine is -3 dBFS).
const FULL_SCALE: i32 = 903;
/// A frame is voiced 9 dB above the noise floor, or 4 dB above it with clearly more zero crossings than the background
/// (fricatives such as «с», «ш» are quiet but hiss). Frames 3 dB above the floor extend an utterance's bounds.
const VOICED_ABOVE: i64 = 90;
const HISS_ABOVE: i64 = 40;
const HISS_CROSSINGS: i64 = (FRAME / 8) as i64;
const WEAK_ABOVE: i64 = 30;
/// Nothing quieter than -50 dBFS (RMS 100) is speech, however quiet the background.
const QUIETEST: i64 = (FULL_SCALE - 500) as i64;
/// The floor follows a quieter background at once and a louder one slowly: at most 3 dB/s while speech may go on.
const FLOOR_SCALE: i64 = 64;
const FLOOR_RISE: i64 = 20;
const SAMPLES_PER_MS: u64 = RATE as u64 / 1000;

/// Tenths of a decibel of a mean square (`10 * log10(energy)`, 0 for 0).
pub fn decibels(energy: u64) -> i32 {
    if energy <= 1 { return 0; }
    let whole = 63 - energy.leading_zeros() as i64;
    let mut mantissa: u64 = if whole >= 30 { energy >> (whole - 30) } else { energy << (30 - whole) }; // [1, 2) in Q30
    let mut fraction = 0i64;
    for bit in (0..16).rev() {
        mantissa = (mantissa * mantissa) >> 30;
        if mantissa >= 2 << 30 { mantissa >>= 1; fraction |= 1 << bit; }
    }
    (((whole << 16) | fraction) * 30_103 / 1_000 >> 16) as i32
}

/// Decibels relative to full scale (rounded, negative) of a level in tenths.
pub fn dbfs(level: i32) -> i32 { let tenths = level - FULL_SCALE; (tenths + if tenths < 0 { -5 } else { 5 }) / 10 }

/// The peak and the RMS of samples in dBFS, as a level meter shows them (000-APP-0053); digital silence is -90.
pub fn meter(samples: &[i16]) -> (i32, i32) {
    if samples.is_empty() { return (dbfs(0), dbfs(0)); }
    let peak = samples.iter().map(|&s| (s as i32).unsigned_abs() as u64).max().unwrap_or(0);
    let square = samples.iter().map(|&s| (s as i64 * s as i64) as u64).sum::<u64>() / samples.len() as u64;
    (dbfs(decibels(peak * peak)), dbfs(decibels(square)))
}

/// A stretch of speech: where it starts in the stream, its 16 kHz mono samples and its level (dBFS of the mean square
/// of its voiced frames).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Utterance { pub start_ms: u64, pub samples: Vec<i16>, pub level: i32 }

impl Utterance {
    pub fn length_ms(&self) -> u64 { self.samples.len() as u64 / SAMPLES_PER_MS }
}

/// Speech detection over a 16 kHz mono stream: frame energy against an adaptive noise floor plus the zero-crossing
/// rate. An utterance starts after `START_FRAMES` voiced frames and ends after `HANGOVER_FRAMES` frames below the
/// threshold; its bounds are the first and last voiced frames, widened over adjacent frames just above the floor (at
/// most `EXTEND_FRAMES`). Utterances shorter than `MIN_MS` are dropped, longer than `MAX_MS` are cut there.
pub struct Detector {
    /// Samples from absolute index `base` (kept back far enough to widen a pending utterance).
    history: Vec<i16>,
    base: u64,
    /// DC-blocked samples not yet analysed, from absolute index `frame * HOP`.
    analysis: Vec<i32>,
    frame: u64,
    dc_in: i32,
    dc_out: i32,
    /// Noise floor (tenths of a dB, times `FLOOR_SCALE`) and background zero crossings per frame (times 16); `None`
    /// before the first frame.
    floor: Option<i64>,
    crossings: i64,
    /// Frames just above the floor, newest in bit 0.
    weak: u64,
    /// Frames before this one belong to an earlier utterance.
    earliest: u64,
    speaking: bool,
    run: usize,
    first: u64,
    last: u64,
    tail: u64,
    quiet: usize,
    energy: u64,
    voiced: u64,
}

impl Default for Detector { fn default() -> Self { Self::new() } }

impl Detector {
    pub fn new() -> Self {
        Self { history: Vec::new(), base: 0, analysis: Vec::new(), frame: 0, dc_in: 0, dc_out: 0, floor: None, crossings: 0, weak: 0,
               earliest: 0, speaking: false, run: 0, first: 0, last: 0, tail: 0, quiet: 0, energy: 0, voiced: 0 }
    }

    /// The current noise floor in dBFS.
    pub fn floor_dbfs(&self) -> i32 { dbfs((self.floor.unwrap_or(0) / FLOOR_SCALE) as i32) }

    /// The level that starts speech now, in dBFS: 9 dB above the noise floor, and never below -50 dBFS (a quieter
    /// frame that hisses counts from 4 dB above the floor).
    pub fn threshold_dbfs(&self) -> i32 { dbfs((self.floor.unwrap_or(0) / FLOOR_SCALE + VOICED_ABOVE).max(QUIETEST) as i32) }

    /// An utterance is in progress.
    pub fn speaking(&self) -> bool { self.speaking }

    /// Feeds 16 kHz mono samples; `found` gets every utterance that ends in them.
    pub fn push(&mut self, samples: &[i16], found: &mut dyn FnMut(Utterance)) {
        self.history.extend_from_slice(samples);
        for &sample in samples {
            // DC blocker: y = x - x1 + 0.995 * y1 (a microphone's offset is not speech).
            let x = sample as i32;
            let y = x - self.dc_in + self.dc_out * 4075 / 4096; // dividing rounds toward 0: no stuck residue
            self.dc_in = x; self.dc_out = y;
            self.analysis.push(y);
        }
        while self.analysis.len() >= FRAME {
            self.analyse(found);
            self.analysis.drain(..HOP);
        }
    }

    /// The stream has ended: a pending utterance ends with it.
    pub fn finish(&mut self, found: &mut dyn FnMut(Utterance)) {
        if self.speaking { self.emit(self.tail, found); }
        self.speaking = false;
        self.run = 0;
    }

    fn analyse(&mut self, found: &mut dyn FnMut(Utterance)) {
        let window = &self.analysis[..FRAME];
        let energy = window.iter().map(|&y| (y as i64 * y as i64) as u64).sum::<u64>() / FRAME as u64;
        let crossings = window.windows(2).filter(|pair| (pair[0] < 0) != (pair[1] < 0)).count() as i64;
        let level = decibels(energy) as i64;
        let k = self.frame;
        self.frame += 1;
        let current = *self.floor.get_or_insert(level * FLOOR_SCALE);
        let floor = current / FLOOR_SCALE;
        let audible = level >= QUIETEST;
        let hiss = level >= floor + HISS_ABOVE && crossings * 16 > self.crossings + HISS_CROSSINGS * 16;
        let voiced = audible && (level >= floor + VOICED_ABOVE || hiss);
        let weak = audible && (voiced || level >= floor + WEAK_ABOVE);
        self.weak = self.weak << 1 | weak as u64;
        // The background: follows quieter frames at once, louder ones slowly (and barely while speech may go on).
        let scaled = level * FLOOR_SCALE;
        let next = if scaled < current { current + (scaled - current) / 4 }
                   else if voiced || self.speaking { current + ((scaled - current) / 32).min(FLOOR_RISE) }
                   else { current + (scaled - current) / 32 };
        self.floor = Some(next);
        if !voiced && !self.speaking { self.crossings += (crossings * 16 - self.crossings) / 16; }
        if !self.speaking {
            if !voiced {
                self.run = 0;
                // Nothing pending: keep what widening the next utterance may need.
                let keep = Self::start((k + 1).saturating_sub(EXTEND_FRAMES).max(self.earliest));
                if keep > self.base { self.forget(keep); }
                return;
            }
            if self.run == 0 { self.first = k; self.energy = 0; self.voiced = 0; }
            self.run += 1;
            self.energy += energy; self.voiced += 1;
            if self.run < START_FRAMES { return; }
            // Speech: widen the start over the quieter frames just before it.
            let mut widened = 0;
            while widened < EXTEND_FRAMES && self.first > self.earliest && self.weak >> (k - self.first + 1) & 1 == 1
                && Self::start(self.first - 1) >= self.base { self.first -= 1; widened += 1; }
            self.speaking = true; self.last = k; self.tail = k; self.quiet = 0;
            return;
        }
        if voiced { self.last = k; self.tail = k; self.quiet = 0; self.energy += energy; self.voiced += 1; }
        else {
            self.quiet += 1;
            if weak && self.tail + 1 == k && k - self.last <= EXTEND_FRAMES { self.tail = k; }
        }
        if self.quiet >= HANGOVER_FRAMES {
            self.emit(self.tail, found);
        } else if Self::end(k) - Self::start(self.first) >= MAX_MS * SAMPLES_PER_MS {
            // Too long: cut here; speech that goes on starts a new utterance.
            self.emit(k, found);
        }
    }

    // A voiced frame stands for its middle 10 ms; utterances start on a whole millisecond.
    fn start(frame: u64) -> u64 { (frame * HOP as u64 + ((FRAME - HOP) / 2) as u64) / SAMPLES_PER_MS * SAMPLES_PER_MS }
    fn end(frame: u64) -> u64 { frame * HOP as u64 + ((FRAME + HOP) / 2) as u64 }

    fn emit(&mut self, last: u64, found: &mut dyn FnMut(Utterance)) {
        let start = Self::start(self.first);
        let end = Self::end(last).min(start + MAX_MS * SAMPLES_PER_MS);
        if end - start >= MIN_MS * SAMPLES_PER_MS && start >= self.base {
            let samples = self.history[(start - self.base) as usize..(end - self.base) as usize].to_vec();
            let level = dbfs(decibels(self.energy / self.voiced.max(1)));
            found(Utterance { start_ms: start / SAMPLES_PER_MS, samples, level });
        }
        self.forget(end);
        self.speaking = false;
        self.run = 0;
        self.earliest = last + 1;
    }

    fn forget(&mut self, upto: u64) {
        let drop = (upto.saturating_sub(self.base) as usize).min(self.history.len());
        self.history.drain(..drop);
        self.base += drop as u64;
    }
}
