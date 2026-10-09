//! Host tests of the dictation engine's features (libmind/src/voice/fbank.rs, 250): equal to kaldi-native-fbank's on a
//! test signal (tests/fbank_reference.txt, made by scripts/voice_dictate/fbank_reference.py), and the elementary
//! functions they use against std's.
extern crate alloc;
#[path = "../libmind/src/voice/math.rs"]
mod math;
#[path = "../libmind/src/voice/fbank.rs"]
mod fbank;

const SAMPLES: usize = 5300;

// The reference script's signal, in integers so both make the same samples.
fn signal() -> Vec<i16> {
    let mut state: u64 = 12345;
    (0..SAMPLES as i64).map(|i| {
        state = (state * 1103515245 + 12345) % (1 << 31);
        let noise = ((state >> 16) % 2001) as i64 - 1000;
        let phase = i % 37;
        let triangle = if phase < 37 / 2 { (phase * 2 * 6000).div_euclid(37) - 6000 } else { 6000 - ((phase - 37 / 2) * 2 * 6000).div_euclid(37) };
        let square = if (i / 1000) % 2 == 0 { if i % 113 < 56 { 3000 } else { -3000 } } else { 0 };
        (triangle + square + noise).clamp(-32768, 32767) as i16
    }).collect()
}

#[test]
fn equal_to_kaldi_native_fbank() {
    let text = std::fs::read_to_string("tests/fbank_reference.txt")
        .unwrap();
    let reference: Vec<Vec<f32>> = text.lines().filter(|l| !l.starts_with('#')).map(|l| l.split(' ').map(|v| v.parse().unwrap()).collect()).collect();
    let features = fbank::Fbank::new().compute_i16(&signal());
    assert_eq!(fbank::Fbank::frames(SAMPLES), reference.len());
    assert_eq!(features.len(), reference.len() * fbank::BANDS);
    let mut worst = 0.0f32;
    for (frame, row) in reference.iter().enumerate() {
        for (band, &want) in row.iter().enumerate() {
            let got = features[frame * fbank::BANDS + band];
            worst = worst.max((got - want).abs());
            assert!((got - want).abs() < 2e-3, "frame {} band {}: {} against {}", frame, band, got, want);
        }
    }
    println!("largest difference {:e}", worst);
}

#[test]
fn elementary_functions() {
    for i in 1..2000 {
        let x = i as f64 * 0.37 - 300.0;
        assert!((math::cos(x) - x.cos()).abs() < 1e-12, "cos {}", x);
        assert!((math::sin(x) - x.sin()).abs() < 1e-12, "sin {}", x);
        let y = (i as f64 * 0.013).exp() * 1e-6;
        assert!((math::ln(y) - y.ln()).abs() < 1e-12 * y.ln().abs().max(1.0), "ln {}", y);
        assert!((math::sqrt(y) - y.sqrt()).abs() <= 1e-15 * y.sqrt().max(1.0), "sqrt {}", y);
        let z = i as f64 * 0.05 - 50.0;
        assert!((math::exp(z) - z.exp()).abs() <= 1e-14 * z.exp(), "exp {}", z);
        let f = (i as f32 * 0.01 - 10.0).exp();
        assert!((math::lnf(f) - f.ln()).abs() <= 2e-7 * f.ln().abs().max(1.0), "lnf {}: {} {}", f, math::lnf(f), f.ln());
        let g = i as f32 * 0.04 - 40.0;
        assert!((math::expf(g) - g.exp()).abs() <= 3e-7 * g.exp(), "expf {}: {} {}", g, math::expf(g), g.exp());
    }
    assert_eq!(math::lnf(1.0), 0.0);
    assert!(math::expf(-200.0) == 0.0 && math::expf(200.0).is_infinite());
    // The edges: subnormals, zero, infinities, NaN, the ends of f32's range.
    for f in [1e-40f32, 1e-45, f32::MIN_POSITIVE, f32::MAX] { assert!((math::lnf(f) - f.ln()).abs() <= 2e-7 * f.ln().abs(), "lnf {}", f); }
    assert!(math::lnf(0.0) == f32::NEG_INFINITY && math::lnf(f32::INFINITY) == f32::INFINITY && math::lnf(-1.0).is_nan() && math::lnf(f32::NAN).is_nan());
    for g in [88.72f32, 88.0, -87.0, -100.0, -103.0] { assert!((math::expf(g) - g.exp()).abs() <= 3e-7 * g.exp() + 1e-45, "expf {}: {} {}", g, math::expf(g), g.exp()); }
    assert!(math::expf(88.73).is_infinite() && math::expf(f32::NAN).is_nan() && math::expf(f32::NEG_INFINITY) == 0.0);
}
