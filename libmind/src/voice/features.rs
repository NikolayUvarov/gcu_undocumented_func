//! Features for the voice recognizer (docs/voice, V1): 40 log-mel band energies of 25 ms frames every 10 ms, computed in
//! integers (a 512-point FFT, triangular mel filters from 64 Hz to 8 kHz, energies in tenths of a decibel), so the
//! host trainer (scripts/voice_train.rs) and the target compute the same numbers. Host tests include this file.
use super::front::{decibels, sin, RATE};
use alloc::vec;
use alloc::vec::Vec;

pub const BANDS: usize = 40;
pub const FRAME: usize = 400;
pub const HOP: usize = 160;
const FFT: usize = 512;
const BINS: usize = FFT / 2 + 1;
const LOW_HZ: f64 = 64.0;
/// Energies are never taken below this (tenths of a dB): digital silence and faint noise look alike.
const FLOOR: i32 = 200;

fn cos(x: f64) -> f64 { sin(x + core::f64::consts::FRAC_PI_2) }
fn mel(hz: f64) -> f64 { 1127.0 * ln(1.0 + hz / 700.0) }
fn mel_to_hz(mel: f64) -> f64 { 700.0 * (exp(mel / 1127.0) - 1.0) }

// ln and exp for the filter design (soft float, once).
fn ln(x: f64) -> f64 {
    // x = m * 2^e with m in [1, 2): ln x = e ln 2 + 2 atanh((m - 1) / (m + 1)).
    let (mut m, mut e) = (x, 0i32);
    while m >= 2.0 { m /= 2.0; e += 1; }
    while m < 1.0 { m *= 2.0; e -= 1; }
    let z = (m - 1.0) / (m + 1.0);
    let (z2, mut term, mut sum) = (z * z, z, 0.0);
    for k in 0..30 { sum += term / (2 * k + 1) as f64; term *= z2; }
    e as f64 * core::f64::consts::LN_2 + 2.0 * sum
}
pub(crate) fn exp(x: f64) -> f64 {
    let n = (x / core::f64::consts::LN_2) as i32;
    let r = x - n as f64 * core::f64::consts::LN_2;
    let (mut term, mut sum) = (1.0, 1.0);
    for k in 1..25 { term *= r / k as f64; sum += term; }
    let mut out = sum;
    if n >= 0 { for _ in 0..n { out *= 2.0; } } else { for _ in 0..-n { out /= 2.0; } }
    out
}

/// The window, FFT twiddles and mel filters, made once.
pub struct Features {
    window: Vec<i32>,
    cos: Vec<i32>,
    sin: Vec<i32>,
    /// Filter b: first bin and Q15 weights.
    filters: Vec<(usize, Vec<u32>)>,
}

impl Default for Features { fn default() -> Self { Self::new() } }

impl Features {
    pub fn new() -> Self {
        let pi = core::f64::consts::PI;
        let window = (0..FRAME).map(|n| ((0.54 - 0.46 * cos(2.0 * pi * n as f64 / (FRAME - 1) as f64)) * 32767.0 + 0.5) as i32).collect();
        let cos_table = (0..FFT / 2).map(|k| { let v = cos(2.0 * pi * k as f64 / FFT as f64) * 32767.0; if v >= 0.0 { (v + 0.5) as i32 } else { (v - 0.5) as i32 } }).collect();
        let sin_table = (0..FFT / 2).map(|k| { let v = -sin(2.0 * pi * k as f64 / FFT as f64) * 32767.0; if v >= 0.0 { (v + 0.5) as i32 } else { (v - 0.5) as i32 } }).collect();
        let (low, high) = (mel(LOW_HZ), mel(RATE as f64 / 2.0));
        let edges: Vec<f64> = (0..BANDS + 2).map(|i| mel_to_hz(low + (high - low) * i as f64 / (BANDS + 1) as f64) * FFT as f64 / RATE as f64).collect();
        let filters = (0..BANDS).map(|b| {
            let (left, center, right) = (edges[b], edges[b + 1], edges[b + 2]);
            let first = left as usize + 1;
            let last = (right as usize).min(BINS - 1);
            let weights = (first..=last.max(first)).map(|k| {
                let k = k as f64;
                let w = if k <= center { (k - left) / (center - left) } else { (right - k) / (right - center) };
                (w.clamp(0.0, 1.0) * 32767.0 + 0.5) as u32
            }).collect();
            (first, weights)
        }).collect();
        Self { window, cos: cos_table, sin: sin_table, filters }
    }

    /// The number of frames in `samples` (16 kHz mono).
    pub fn frames(samples: usize) -> usize { if samples < FRAME { 0 } else { (samples - FRAME) / HOP + 1 } }

    /// Log-mel energies (tenths of a dB) of every frame of `samples`, frame after frame, `BANDS` per frame.
    pub fn log_mel(&self, samples: &[i16]) -> Vec<i16> {
        let frames = Self::frames(samples.len());
        let mut out = Vec::with_capacity(frames * BANDS);
        let (mut re, mut im) = (vec![0i32; FFT], vec![0i32; FFT]);
        let mut power = vec![0u64; BINS];
        for f in 0..frames {
            let at = f * HOP;
            // Pre-emphasis (0.97) and the Hamming window.
            for n in 0..FFT {
                re[n] = if n < FRAME {
                    let x = samples[at + n] as i32;
                    let previous = if at + n > 0 { samples[at + n - 1] as i32 } else { 0 };
                    let emphasized = (x - (previous * 31785 >> 15)).clamp(-32768, 32767);
                    emphasized * self.window[n] >> 15
                } else { 0 };
                im[n] = 0;
            }
            self.fft(&mut re, &mut im);
            for k in 0..BINS { let (r, i) = (re[k] as i64, im[k] as i64); power[k] = ((r * r + i * i) >> 8) as u64; }
            for (first, weights) in &self.filters {
                let energy: u128 = weights.iter().enumerate().map(|(i, &w)| power[first + i] as u128 * w as u128).sum();
                let energy = (energy >> 15).min(u64::MAX as u128) as u64;
                out.push(decibels(energy).max(FLOOR) as i16);
            }
        }
        out
    }

    // In-place radix-2 FFT of 512 points; inputs up to 2^15 grow at most 512-fold, which i32 holds.
    fn fft(&self, re: &mut [i32], im: &mut [i32]) {
        let mut j = 0usize;
        for i in 1..FFT {
            let mut bit = FFT >> 1;
            while j & bit != 0 { j ^= bit; bit >>= 1; }
            j |= bit;
            if i < j { re.swap(i, j); im.swap(i, j); }
        }
        let mut size = 2;
        while size <= FFT {
            let step = FFT / size;
            for start in (0..FFT).step_by(size) {
                for k in 0..size / 2 {
                    let (wr, wi) = (self.cos[k * step] as i64, self.sin[k * step] as i64);
                    let (a, b) = (start + k, start + k + size / 2);
                    let (xr, xi) = (re[b] as i64, im[b] as i64);
                    let tr = ((xr * wr - xi * wi) >> 15) as i32;
                    let ti = ((xr * wi + xi * wr) >> 15) as i32;
                    re[b] = re[a] - tr; im[b] = im[a] - ti;
                    re[a] += tr; im[a] += ti;
                }
            }
            size *= 2;
        }
    }
}

/// Subtracts every band's mean over the utterance (cepstral mean normalization in the log-mel domain): the level and
/// the microphone's colour do not matter.
pub fn normalize(features: &mut [i16]) {
    let frames = features.len() / BANDS;
    if frames == 0 { return; }
    for b in 0..BANDS {
        let mean = (0..frames).map(|f| features[f * BANDS + b] as i64).sum::<i64>() / frames as i64;
        for f in 0..frames { features[f * BANDS + b] = (features[f * BANDS + b] as i64 - mean) as i16; }
    }
}
