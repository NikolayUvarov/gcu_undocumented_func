// Q30 fixed point: cosine and exponential for coefficients, resonators, noise and the glottal source.
pub const RATE: i64 = 16_000;
const Q: i64 = 1 << 30;
const PI: i64 = 3_373_259_426; // π · 2^30

pub fn cos_q30(x: i64) -> i64 {
    let mut x = x.rem_euclid(2 * PI);
    if x > PI { x = 2 * PI - x; }
    let negative = x > PI / 2;
    if negative { x = PI - x; }
    let x2 = x * x >> 30;
    let mut t = Q - x2 / 90;
    for divisor in [56, 30, 12, 2] { t = Q - (x2 * t >> 30) / divisor; }
    if negative { -t } else { t }
}

fn exp_neg_q30(y: i64) -> i64 {
    let mut t = Q - y / 6;
    for divisor in [5, 4, 3, 2, 1] { t = Q - (y * t >> 30) / divisor; }
    t
}

// Second-order resonator (Klatt): unity gain at DC for the cascade, or at the peak for the parallel branch.
#[derive(Clone, Copy, Default)]
pub struct Resonator { a: i64, b: i64, c: i64, y1: i64, y2: i64 }

impl Resonator {
    pub fn set(&mut self, frequency: i32, bandwidth: i32, peak: bool) {
        let theta = 2 * PI * frequency.clamp(50, 7_900) as i64 / RATE;
        let r = exp_neg_q30(PI * bandwidth.clamp(20, 8_000) as i64 / RATE);
        self.c = -(r * r >> 30);
        self.b = 2 * (r * cos_q30(theta) >> 30);
        self.a = if peak { ((Q - r) * (2 * cos_q30(PI / 2 - theta)).abs().max(Q / 8)) >> 30 } else { Q - self.b - self.c };
    }
    pub fn run(&mut self, x: i64) -> i64 {
        let y = (self.a * x + self.b * self.y1 + self.c * self.y2) >> 30;
        self.y2 = self.y1; self.y1 = y;
        y
    }
    /// Forgets the past output (the coefficients stay).
    pub fn clear(&mut self) { self.y1 = 0; self.y2 = 0; }
}

// Antiresonator (spectral zero): inverse filter of a resonator with unity gain at DC.
#[derive(Clone, Copy, Default)]
pub struct Antiresonator { a: i64, b: i64, c: i64, x1: i64, x2: i64 }

impl Antiresonator {
    pub fn set(&mut self, frequency: i32, bandwidth: i32) {
        let mut r = Resonator::default(); r.set(frequency, bandwidth, false);
        self.a = (Q << 30) / r.a; self.b = -(r.b << 30) / r.a; self.c = -(r.c << 30) / r.a;
    }
    pub fn run(&mut self, x: i64) -> i64 {
        let y = (self.a * x + self.b * self.x1 + self.c * self.x2) >> 30;
        self.x2 = self.x1; self.x1 = x;
        y
    }
    pub fn clear(&mut self) { self.x1 = 0; self.x2 = 0; }
}

pub struct Noise(u32);
impl Noise {
    pub const fn new() -> Self { Self(0x1234_5678) }
    // Uniform noise in ±4096.
    pub fn next(&mut self) -> i64 { self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345); ((self.0 >> 16) as i64 & 0x1FFF) - 4096 }
}

// KLGLOTT88 source: flow derivative 2x − 3x² in the open phase; the abrupt closure provides the excitation.
pub struct Glottis { phase: u32 }
impl Glottis {
    pub const fn new() -> Self { Self { phase: 0 } }
    /// Returns a sample (±4096 at amplitude 4096) and whether a new period has started.
    pub fn next(&mut self, f0: i64) -> (i64, bool) {
        let step = ((f0.clamp(50, 400) << 32) / RATE) as u32;
        let (phase, wrapped) = self.phase.overflowing_add(step);
        self.phase = phase;
        let open = (u32::MAX as u64 * 6 / 10) as u32; // open phase is 60 % of the period
        if phase >= open { return (0, wrapped); }
        let x = ((phase as i64) << 15) / open as i64; // Q15
        (((2 * x - (3 * x * x >> 15)) * 4096) >> 15, wrapped)
    }
}
