//! Log-mel filter bank features as Kaldi computes them (kaldi-native-fbank with sherpa-onnx's settings), the input of
//! the dictation models (250): 80 bands from 20 Hz to 7.6 kHz of 25 ms frames every 10 ms; per frame the DC offset
//! removed, pre-emphasis 0.97, a Povey window, a 512-point FFT and the power spectrum; no dither; frames centred on
//! their shift, the signal reflected at its ends (Kaldi's `snip_edges = false`). In f32, the design once in f64.
//! Host tests include this file (tests/fbank_host.rs).
use super::math;
use alloc::vec;
use alloc::vec::Vec;

/// Mel bands per frame.
pub const BANDS: usize = 80;
/// Samples between frames (10 ms at 16 kHz).
pub const SHIFT: usize = 160;
/// Samples in a frame (25 ms).
pub const LENGTH: usize = 400;
const FFT: usize = 512;
const RATE: f64 = 16_000.0;
const LOW_HZ: f64 = 20.0;
const HIGH_HZ: f64 = RATE / 2.0 - 400.0;
const PREEMPHASIS: f32 = 0.97;

fn mel(hz: f64) -> f64 { 1127.0 * math::ln(1.0 + hz / 700.0) }

/// The window, the FFT's twiddles and the mel filters, made once.
pub struct Fbank {
    window: Vec<f32>,
    twiddles: Vec<(f32, f32)>,
    /// Per band: its first FFT bin and its weights from there.
    banks: Vec<(usize, Vec<f32>)>,
}

impl Default for Fbank { fn default() -> Self { Self::new() } }

impl Fbank {
    pub fn new() -> Self {
        let a = 2.0 * core::f64::consts::PI / (LENGTH - 1) as f64;
        let window = (0..LENGTH).map(|i| math::pow(0.5 - 0.5 * math::cos(a * i as f64), 0.85) as f32).collect();
        let twiddles = (0..FFT / 2).map(|k| {
            let angle = -2.0 * core::f64::consts::PI * k as f64 / FFT as f64;
            (math::cos(angle) as f32, math::sin(angle) as f32)
        }).collect();
        // Kaldi's MelBanks: triangles evenly spaced in mel over the first FFT / 2 bins, weighed by each bin's mel.
        let (low, high) = (mel(LOW_HZ), mel(HIGH_HZ));
        let delta = (high - low) / (BANDS + 1) as f64;
        let bin_hz = RATE / FFT as f64;
        let bin_mels: Vec<f64> = (0..FFT / 2).map(|i| mel(bin_hz * i as f64)).collect();
        let banks = (0..BANDS).map(|band| {
            let (left, centre, right) = (low + band as f64 * delta, low + (band + 1) as f64 * delta, low + (band + 2) as f64 * delta);
            let weights: Vec<(usize, f32)> = bin_mels.iter().enumerate().filter(|(_, &m)| m > left && m < right)
                .map(|(i, &m)| (i, if m <= centre { (m - left) / (centre - left) } else { (right - m) / (right - centre) } as f32)).collect();
            let first = weights.first().map_or(0, |w| w.0);
            (first, weights.iter().map(|w| w.1).collect())
        }).collect();
        Self { window, twiddles, banks }
    }

    /// Frames of `samples` samples.
    pub fn frames(samples: usize) -> usize { (samples + SHIFT / 2) / SHIFT }

    /// The features of `samples` (16 kHz, scaled to [-1, 1)), `BANDS` per frame.
    pub fn compute(&self, samples: &[f32]) -> Vec<f32> {
        let frames = Self::frames(samples.len());
        let mut out = vec![0.0f32; frames * BANDS];
        let (mut re, mut im, mut power) = (vec![0.0f32; FFT], vec![0.0f32; FFT], vec![0.0f32; FFT / 2]);
        for (frame, bands) in out.chunks_exact_mut(BANDS).enumerate() {
            self.frame(samples, frame, &mut re, &mut im, &mut power);
            for (value, (first, weights)) in bands.iter_mut().zip(&self.banks) {
                let energy: f32 = weights.iter().zip(&power[*first..]).map(|(w, p)| w * p).sum();
                *value = math::lnf(energy.max(f32::EPSILON));
            }
        }
        out
    }

    /// The same for 16-bit samples.
    pub fn compute_i16(&self, samples: &[i16]) -> Vec<f32> {
        let scaled: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
        self.compute(&scaled)
    }

    // The power spectrum of frame `frame` into `power` (its first FFT / 2 bins).
    fn frame(&self, samples: &[f32], frame: usize, re: &mut [f32], im: &mut [f32], power: &mut [f32]) {
        let n = samples.len() as isize;
        let start = (frame * SHIFT + SHIFT / 2) as isize - (LENGTH / 2) as isize;
        for (i, value) in re[..LENGTH].iter_mut().enumerate() {
            let mut s = start + i as isize;
            while s < 0 || s >= n { s = if s < 0 { -s - 1 } else { 2 * n - 1 - s }; }
            *value = samples[s as usize];
        }
        let mean = re[..LENGTH].iter().sum::<f32>() / LENGTH as f32;
        for value in re[..LENGTH].iter_mut() { *value -= mean; }
        for i in (1..LENGTH).rev() { re[i] -= PREEMPHASIS * re[i - 1]; }
        re[0] -= PREEMPHASIS * re[0];
        for (value, w) in re[..LENGTH].iter_mut().zip(&self.window) { *value *= w; }
        re[LENGTH..].fill(0.0);
        im.fill(0.0);
        self.fft(re, im);
        for (i, p) in power.iter_mut().enumerate() { *p = re[i] * re[i] + im[i] * im[i]; }
    }

    // In place, radix 2, decimation in time.
    fn fft(&self, re: &mut [f32], im: &mut [f32]) {
        let mut j = 0;
        for i in 1..FFT {
            let mut bit = FFT >> 1;
            while j & bit != 0 { j ^= bit; bit >>= 1; }
            j |= bit;
            if i < j { re.swap(i, j); im.swap(i, j); }
        }
        let mut size = 2;
        while size <= FFT {
            let (half, step) = (size / 2, FFT / size);
            for start in (0..FFT).step_by(size) {
                for k in 0..half {
                    let (wr, wi) = self.twiddles[k * step];
                    let (a, b) = (start + k, start + k + half);
                    let (tr, ti) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                    re[b] = re[a] - tr; im[b] = im[a] - ti;
                    re[a] += tr; im[a] += ti;
                }
            }
            size *= 2;
        }
    }
}
