//! Elementary functions for the dictation engine (250): `core` has no ln, exp, cos or sqrt without std. The f64 ones
//! design filters and windows once; the f32 ones run per frame and per activation, to about 1e-7 relative error.
//! Host tests include this file.

const LN_2: f64 = core::f64::consts::LN_2;

/// ln x for x > 0 (x <= 0: -inf).
pub fn ln(x: f64) -> f64 {
    if x <= 0.0 { return f64::NEG_INFINITY; }
    // x = m * 2^e with m in [sqrt(1/2), sqrt(2)): ln x = e ln 2 + 2 atanh((m - 1) / (m + 1)).
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7FF) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000F_FFFF_FFFF_FFFF) | 0x3FF0_0000_0000_0000);
    if e == -1023 { return ln(x * 4503599627370496.0) - 52.0 * LN_2; } // subnormal
    if m > core::f64::consts::SQRT_2 { m /= 2.0; e += 1; }
    let z = (m - 1.0) / (m + 1.0);
    let (z2, mut term, mut sum) = (z * z, z, 0.0);
    for k in 0..20 { sum += term / (2 * k + 1) as f64; term *= z2; }
    e as f64 * LN_2 + 2.0 * sum
}

/// e^x.
pub fn exp(x: f64) -> f64 {
    if x > 709.0 { return f64::INFINITY; }
    if x < -745.0 { return 0.0; }
    // x = n ln 2 + r with |r| <= ln 2 / 2.
    let n = (x / LN_2 + if x < 0.0 { -0.5 } else { 0.5 }) as i64;
    let r = x - n as f64 * LN_2;
    let (mut term, mut sum) = (1.0, 1.0);
    for k in 1..20 { term *= r / k as f64; sum += term; }
    scale(sum, n)
}

// x * 2^n.
fn scale(mut x: f64, mut n: i64) -> f64 {
    while n > 1000 { x *= f64::from_bits(0x7E70_0000_0000_0000); n -= 1000; } // 2^1000
    while n < -1000 { x *= f64::from_bits(0x0170_0000_0000_0000); n += 1000; } // 2^-1000
    x * f64::from_bits(((n + 1023) as u64) << 52)
}

/// cos x.
pub fn cos(x: f64) -> f64 {
    let pi = core::f64::consts::PI;
    let turns = x / (2.0 * pi);
    let mut r = x - (turns as i64) as f64 * 2.0 * pi;
    if r > pi { r -= 2.0 * pi } else if r < -pi { r += 2.0 * pi }
    let r = if r < 0.0 { -r } else { r };
    // cos on [0, pi]: cos r = -cos(pi - r) keeps the series short.
    let (r, sign) = if r > pi / 2.0 { (pi - r, -1.0) } else { (r, 1.0) };
    let r2 = r * r;
    let (mut term, mut sum) = (1.0, 1.0);
    for n in 1..14 { term = -term * r2 / ((2 * n - 1) as f64 * (2 * n) as f64); sum += term; }
    sign * sum
}

/// sin x.
pub fn sin(x: f64) -> f64 { cos(x - core::f64::consts::FRAC_PI_2) }

/// sqrt x for x >= 0.
pub fn sqrt(x: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    let mut y = f64::from_bits((x.to_bits() >> 1) + (0x3FF0_0000_0000_0000 >> 1)); // a first guess from the exponent
    for _ in 0..6 { y = 0.5 * (y + x / y); }
    y
}

/// x^y for x >= 0.
pub fn pow(x: f64, y: f64) -> f64 { if x <= 0.0 { 0.0 } else { exp(y * ln(x)) } }

// 1.5 * 2^23: x + MAGIC - MAGIC rounds |x| < 2^22 to an integer, ties to even, and t = x + MAGIC holds it in its
// low mantissa bits.
const MAGIC: f32 = 12_582_912.0;

/// ln x in f32 for x > 0 (0: -inf, below 0 or NaN: NaN). Without branches, so that a loop over it vectorizes.
#[inline]
pub fn lnf(x: f32) -> f32 {
    let tiny = x < f32::MIN_POSITIVE;
    let y = if tiny { x * 8388608.0 } else { x }; // subnormals scaled by 2^23
    let bits = y.to_bits() as i32;
    let e = ((bits >> 23) & 0xFF) - 127 - if tiny { 23 } else { 0 };
    let m = f32::from_bits(((bits & 0x007F_FFFF) | 0x3F80_0000) as u32);
    let big = m > core::f32::consts::SQRT_2;
    let (m, e) = (if big { m * 0.5 } else { m }, if big { e + 1 } else { e });
    let z = (m - 1.0) / (m + 1.0);
    let z2 = z * z;
    // 2 atanh z for |z| <= 0.172: six terms are below f32's precision.
    let s = z * (2.0 + z2 * (2.0 / 3.0 + z2 * (2.0 / 5.0 + z2 * (2.0 / 7.0 + z2 * (2.0 / 9.0 + z2 * (2.0 / 11.0))))));
    let out = e as f32 * core::f32::consts::LN_2 + s;
    if x > 0.0 && x < f32::INFINITY { out } else if x == 0.0 { f32::NEG_INFINITY } else if x == f32::INFINITY { x } else { f32::NAN }
}

/// e^x in f32, without branches like lnf.
#[inline]
pub fn expf(x: f32) -> f32 {
    let c = x.max(-104.0).min(89.0);
    let t = c * core::f32::consts::LOG2_E + MAGIC;
    let n = t - MAGIC;
    let r = (c - n * 0.693_145_75) - n * 1.428_606_8e-6; // ln 2 in two parts (Cody and Waite)
    // |r| <= 0.347: the Taylor series to r^7.
    let p = 1.0 + r * (1.0 + r * (0.5 + r * (1.0 / 6.0 + r * (1.0 / 24.0 + r * (1.0 / 120.0 + r * (1.0 / 720.0 + r * (1.0 / 5040.0)))))));
    // 2^n in two factors, each a normal number for n in [-150, 129].
    let k = t.to_bits() as i32 - MAGIC.to_bits() as i32;
    let half = k >> 1;
    let out = p * f32::from_bits(((half + 127) << 23) as u32) * f32::from_bits(((k - half + 127) << 23) as u32);
    if x.is_nan() { x } else { out }
}

/// The error function: its series below 2, else 1 - erfc from erfc's continued fraction; to about 1e-15.
pub fn erf(x: f64) -> f64 {
    let a = if x < 0.0 { -x } else { x };
    let r = if a < 2.0 {
        let (x2, mut term, mut sum) = (a * a, a, a);
        for n in 1..60 {
            term *= -x2 / n as f64;
            let next = term / (2 * n + 1) as f64;
            sum += next;
            if next.abs() < 1e-17 { break; }
        }
        sum * 2.0 / sqrt(core::f64::consts::PI)
    } else if a < 6.0 {
        // erfc a = e^(-a^2) / sqrt(pi) / (a + (1/2) / (a + 1 / (a + (3/2) / (a + ...)))).
        let mut t = a;
        for k in (1..=80).rev() { t = a + (k as f64 / 2.0) / t; }
        1.0 - exp(-a * a) / (sqrt(core::f64::consts::PI) * t)
    } else {
        1.0
    };
    if x < 0.0 { -r } else if x.is_nan() { x } else { r }
}

/// ln(1 + x) for x > -1, exact near 0.
pub fn ln_1p(x: f64) -> f64 {
    if x.abs() < 1e-4 { x - x * x / 2.0 + x * x * x / 3.0 } else { ln(1.0 + x) }
}
