//! An SBC encoder (A2DP 1.3.2, Appendix B, "Technical Specification of SBC"): the polyphase analysis filter of
//! figure 12.5 in fixed point, scale factors (12.7.2), joint stereo by the simple criterion of 12.7.3, the bit
//! allocation of 12.6.3 exactly, quantization (12.7.5) and the frame's syntax (12.4) with its CRC-8 (12.6.1.1). Written
//! from the specification; the prototype filters are its tables 12.23 and 12.24 scaled by 2^30, the matrices
//! cos((i + 0.5)(k - M/2)π/M) by 2^30.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Mono = 0, Dual = 1, Stereo = 2, Joint = 3 }

/// A stream's settings: the sampling frequency's index (0: 16 kHz, 1: 32, 2: 44.1, 3: 48), blocks (4, 8, 12, 16),
/// the channel mode, SNR (else loudness) allocation, subbands (4, 8) and the bitpool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config { pub rate: u8, pub blocks: u8, pub mode: Mode, pub snr: bool, pub subbands: u8, pub bitpool: u8 }

pub const RATES: [u32; 4] = [16_000, 32_000, 44_100, 48_000];
pub const SYNC: u8 = 0x9C;

impl Config {
    pub fn channels(&self) -> usize { if self.mode == Mode::Mono { 1 } else { 2 } }
    pub fn hz(&self) -> u32 { RATES[self.rate as usize & 3] }
    /// Samples per channel in a frame.
    pub fn samples(&self) -> usize { self.blocks as usize * self.subbands as usize }
    /// Bytes in a frame (12.9).
    pub fn frame_len(&self) -> usize {
        let (m, b, ch, pool) = (self.subbands as usize, self.blocks as usize, self.channels(), self.bitpool as usize);
        let head = 4 + (4 * m * ch) / 8;
        match self.mode {
            Mode::Mono | Mode::Dual => head + (b * ch * pool).div_ceil(8),
            Mode::Stereo => head + (b * pool).div_ceil(8),
            Mode::Joint => head + (m + b * pool).div_ceil(8),
        }
    }
    /// The bitrate in bits per second.
    pub fn bitrate(&self) -> u32 { (8 * self.frame_len() as u64 * self.hz() as u64 / self.samples() as u64) as u32 }
    /// The largest bitpool the mode allows (12.5.1).
    pub fn max_bitpool(&self) -> u8 { (if matches!(self.mode, Mode::Mono | Mode::Dual) { 16 } else { 32 } * self.subbands as u16).min(255) as u8 }
}

const PROTO_4: [i32; 40] = [
    0, 576115, 1601898, 2935298, 4120164, 4179058, 2003406, -3285782,
    11718563, 21945682, 31005089, 34567968, 27784982, 6584670, -30947094, -83372133,
    145592169, 209366600, 264824099, 302610729, 316018681, 302610729, 264824099, 209366600,
    -145592169, -83372133, -30947094, 6584670, 27784982, 34567968, 31005089, 21945682,
    -11718563, -3285782, 2003406, 4179058, 4120164, 2935298, 1601898, 576115,
];
const PROTO_8: [i32; 80] = [
    0, 168122, 368569, 595519, 884677, 1223985, 1585274, 1915252,
    2160181, 2258852, 2141627, 1735771, 968681, -191991, -1771385, -3755063,
    6076836, 8621515, 11229669, 13687238, 15733030, 17077392, 17417002, 16448018,
    13891192, 9510749, 3139712, -5278279, -15720018, -28035265, -41956610, -57109432,
    73013309, 89104205, 104770779, 119396536, 132354301, 143091576, 151132925, 156111160,
    157791803, 156111160, 151132925, 143091576, 132354301, 119396536, 104770779, 89104205,
    -73013309, -57109432, -41956610, -28035265, -15720018, -5278279, 3139712, 9510749,
    13891192, 16448018, 17417002, 17077392, 15733030, 13687238, 11229669, 8621515,
    -6076836, -3755063, -1771385, -191991, 968681, 1735771, 2141627, 2258852,
    2160181, 1915252, 1585274, 1223985, 884677, 595519, 368569, 168122,
];
const COS_4: [i32; 32] = [
    759250125, 992008094, 1073741824, 992008094, 759250125, 410903207, 0, -410903207,
    -759250125, 410903207, 1073741824, 410903207, -759250125, -992008094, 0, 992008094,
    -759250125, -410903207, 1073741824, -410903207, -759250125, 992008094, 0, -992008094,
    759250125, -992008094, 1073741824, -992008094, 759250125, -410903207, 0, 410903207,
];
const COS_8: [i32; 128] = [
    759250125, 892783698, 992008094, 1053110176, 1073741824, 1053110176, 992008094, 892783698,
    759250125, 596538995, 410903207, 209476638, 0, -209476638, -410903207, -596538995,
    -759250125, -209476638, 410903207, 892783698, 1073741824, 892783698, 410903207, -209476638,
    -759250125, -1053110176, -992008094, -596538995, 0, 596538995, 992008094, 1053110176,
    -759250125, -1053110176, -410903207, 596538995, 1073741824, 596538995, -410903207, -1053110176,
    -759250125, 209476638, 992008094, 892783698, 0, -892783698, -992008094, -209476638,
    759250125, -596538995, -992008094, 209476638, 1073741824, 209476638, -992008094, -596538995,
    759250125, 892783698, -410903207, -1053110176, 0, 1053110176, 410903207, -892783698,
    759250125, 596538995, -992008094, -209476638, 1073741824, -209476638, -992008094, 596538995,
    759250125, -892783698, -410903207, 1053110176, 0, -1053110176, 410903207, 892783698,
    -759250125, 1053110176, -410903207, -596538995, 1073741824, -596538995, -410903207, 1053110176,
    -759250125, -209476638, 992008094, -892783698, 0, 892783698, -992008094, 209476638,
    -759250125, 209476638, 410903207, -892783698, 1073741824, -892783698, 410903207, 209476638,
    -759250125, 1053110176, -992008094, 596538995, 0, -596538995, 992008094, -1053110176,
    759250125, -892783698, 992008094, -1053110176, 1073741824, -1053110176, 992008094, -892783698,
    759250125, -596538995, 410903207, -209476638, 0, 209476638, -410903207, 596538995,
];

/// Table 12.21 and 12.22, by sampling frequency then subband.
const OFFSET_4: [[i8; 4]; 4] = [[-1, 0, 0, 0], [-2, 0, 0, 1], [-2, 0, 0, 1], [-2, 0, 0, 1]];
const OFFSET_8: [[i8; 8]; 4] = [[-2, 0, 0, 0, 0, 0, 0, 1], [-3, 0, 0, 0, 0, 0, 1, 2], [-4, 0, 0, 0, 0, 0, 1, 2], [-4, 0, 0, 0, 0, 0, 1, 2]];

/// The bit allocation (12.6.3): `bits[ch][sb]` from the scale factors, the same in encoder and decoder.
pub fn allocate(cfg: &Config, scale: &[[u8; 8]; 2], bits: &mut [[u8; 8]; 2]) {
    let (m, pool) = (cfg.subbands as usize, cfg.bitpool as i32);
    let need = |ch: usize, sb: usize| -> i32 {
        let sf = scale[ch][sb] as i32;
        if cfg.snr { return sf; }
        if sf == 0 { return -5; }
        let loudness = sf - if m == 4 { OFFSET_4[cfg.rate as usize & 3][sb] } else { OFFSET_8[cfg.rate as usize & 3][sb] } as i32;
        if loudness > 0 { loudness / 2 } else { loudness }
    };
    // Mono and dual channel: each channel alone; stereo and joint stereo: both together.
    let groups: &[&[usize]] = match cfg.mode { Mode::Mono => &[&[0]], Mode::Dual => &[&[0], &[1]], _ => &[&[0, 1]] };
    for &chs in groups {
        let mut bitneed = [[0i32; 8]; 2];
        let mut max = 0;
        for &ch in chs { for sb in 0..m { bitneed[ch][sb] = need(ch, sb); max = max.max(bitneed[ch][sb]); } }
        let (mut bitcount, mut slicecount, mut bitslice) = (0, 0, max + 1);
        loop {
            bitslice -= 1;
            bitcount += slicecount;
            slicecount = 0;
            for &ch in chs {
                for sb in 0..m {
                    let n = bitneed[ch][sb];
                    if n > bitslice + 1 && n < bitslice + 16 { slicecount += 1; } else if n == bitslice + 1 { slicecount += 2; }
                }
            }
            if bitcount + slicecount >= pool { break; }
        }
        if bitcount + slicecount == pool { bitcount += slicecount; bitslice -= 1; }
        for &ch in chs {
            for sb in 0..m { bits[ch][sb] = if bitneed[ch][sb] < bitslice + 2 { 0 } else { (bitneed[ch][sb] - bitslice).min(16) as u8 }; }
        }
        // The rest from subband 0, the channels in turn.
        let order = |k: usize| if chs.len() == 2 { (chs[k % 2], k / 2) } else { (chs[0], k) };
        let steps = chs.len() * m;
        let mut k = 0;
        while bitcount < pool && k < steps {
            let (ch, sb) = order(k);
            if bits[ch][sb] >= 2 && bits[ch][sb] < 16 { bits[ch][sb] += 1; bitcount += 1; }
            else if bitneed[ch][sb] == bitslice + 1 && pool > bitcount + 1 { bits[ch][sb] = 2; bitcount += 2; }
            k += 1;
        }
        k = 0;
        while bitcount < pool && k < steps {
            let (ch, sb) = order(k);
            if bits[ch][sb] < 16 { bits[ch][sb] += 1; bitcount += 1; }
            k += 1;
        }
    }
}

/// Bits written most significant first.
struct Bits<'a> { out: &'a mut [u8], at: usize }

impl Bits<'_> {
    fn put(&mut self, value: u32, n: u8) {
        for i in (0..n).rev() {
            let (byte, bit) = (self.at / 8, 7 - self.at % 8);
            if byte < self.out.len() {
                if value >> i & 1 != 0 { self.out[byte] |= 1 << bit; } else { self.out[byte] &= !(1 << bit); }
            }
            self.at += 1;
        }
    }
}

/// CRC-8 with G(X) = X^8 + X^4 + X^3 + X^2 + 1 from $0F over the first `count` bits of `data` (12.6.1.1).
pub fn crc8(data: &[u8], count: usize) -> u8 {
    let mut crc: u8 = 0x0F;
    for i in 0..count {
        let bit = data[i / 8] >> (7 - i % 8) & 1;
        let top = crc >> 7 ^ bit;
        crc <<= 1;
        if top != 0 { crc ^= 0x1D; }
    }
    crc
}

/// The encoder: each channel's last L = 10 M input samples, newest first.
pub struct Encoder { pub cfg: Config, x: [[i32; 80]; 2] }

/// The scale factor of the subband samples (in 1/32768 of a PCM step) whose largest magnitude is `max`.
fn scale_of(max: i64) -> u8 {
    let mut sf = 0;
    while sf < 15 && max >= 1i64 << (sf + 16) { sf += 1; }
    sf
}

impl Encoder {
    pub fn new(cfg: Config) -> Self { Self { cfg, x: [[0; 80]; 2] } }

    /// One block's subband samples of channel `ch` from its next M input samples (figure 12.5), in 1/32768 PCM steps.
    fn analyse(&mut self, ch: usize, input: impl Iterator<Item = i16>, s: &mut [i64; 8]) {
        let m = self.cfg.subbands as usize;
        let l = 10 * m;
        let x = &mut self.x[ch];
        x.copy_within(0..l - m, m);
        for (i, v) in input.enumerate().take(m) { x[m - 1 - i] = v as i32; }
        let (proto, cos): (&[i32], &[i32]) = if m == 4 { (&PROTO_4, &COS_4) } else { (&PROTO_8, &COS_8) };
        let mut y = [0i64; 16];
        for (i, yi) in y.iter_mut().enumerate().take(2 * m) {
            let mut acc = 0i64;
            for k in 0..5 { let at = i + k * 2 * m; acc += proto[at] as i64 * x[at] as i64; }
            *yi = acc >> 15;
        }
        for (i, si) in s.iter_mut().enumerate().take(m) {
            let mut acc = 0i64;
            for k in 0..2 * m { acc += cos[i * 2 * m + k] as i64 * y[k]; }
            *si = acc >> 30;
        }
    }

    /// One frame from `pcm`, interleaved (`samples()` per channel), into `out`; the frame's length.
    pub fn frame(&mut self, pcm: &[i16], out: &mut [u8]) -> usize {
        let cfg = self.cfg;
        let (m, blocks, chs) = (cfg.subbands as usize, cfg.blocks as usize, cfg.channels());
        let mut sb = [[[0i64; 8]; 2]; 16];
        for (blk, sbb) in sb.iter_mut().enumerate().take(blocks) {
            for (ch, s) in sbb.iter_mut().enumerate().take(chs) {
                let start = blk * m * chs + ch;
                let input = (0..m).map(|i| pcm.get(start + i * chs).copied().unwrap_or(0));
                self.analyse(ch, input, s);
            }
        }
        let mut scale = [[0u8; 8]; 2];
        let max_of = |sb: &[[[i64; 8]; 2]; 16], ch: usize, s: usize| (0..blocks).map(|b| sb[b][ch][s].abs()).max().unwrap_or(0);
        for (ch, sc) in scale.iter_mut().enumerate().take(chs) { for (s, v) in sc.iter_mut().enumerate().take(m) { *v = scale_of(max_of(&sb, ch, s)); } }
        // Joint stereo: a subband (but the last) is sent as sum and difference when their scale factors add up to less.
        let mut join = [false; 8];
        if cfg.mode == Mode::Joint {
            for (s, joined) in join.iter_mut().enumerate().take(m - 1) {
                let (mut ms, mut ss) = (0i64, 0i64);
                for b in sb.iter().take(blocks) { ms = ms.max(((b[0][s] + b[1][s]) >> 1).abs()); ss = ss.max(((b[0][s] - b[1][s]) >> 1).abs()); }
                let (fm, fs) = (scale_of(ms), scale_of(ss));
                if (fm as u32 + fs as u32) < (scale[0][s] as u32 + scale[1][s] as u32) {
                    *joined = true;
                    for b in sb.iter_mut().take(blocks) { let (l, r) = (b[0][s], b[1][s]); b[0][s] = (l + r) >> 1; b[1][s] = (l - r) >> 1; }
                    (scale[0][s], scale[1][s]) = (fm, fs);
                }
            }
        }
        let mut bits = [[0u8; 8]; 2];
        allocate(&cfg, &scale, &mut bits);
        let len = cfg.frame_len();
        let out = &mut out[..len];
        out.fill(0);
        let blocks_code = (blocks / 4 - 1) as u32;
        let mut w = Bits { out, at: 0 };
        w.put(SYNC as u32, 8);
        w.put((cfg.rate as u32) << 6 | blocks_code << 4 | (cfg.mode as u32) << 2 | (cfg.snr as u32) << 1 | (m == 8) as u32, 8);
        w.put(cfg.bitpool as u32, 8);
        w.put(0, 8);
        if cfg.mode == Mode::Joint { for &j in join.iter().take(m) { w.put(j as u32, 1); } }
        for sc in scale.iter().take(chs) { for &v in sc.iter().take(m) { w.put(v as u32, 4); } }
        // The CRC covers the header after the sync word but its own byte, the join bits and the scale factors.
        let covered = w.at;
        let mut crc_in = [0u8; 16];
        let mut c = Bits { out: &mut crc_in, at: 0 };
        let src = &w.out;
        c.put(src[1] as u32, 8);
        c.put(src[2] as u32, 8);
        for i in 32..covered { c.put((src[i / 8] >> (7 - i % 8) & 1) as u32, 1); }
        let crc = crc8(&crc_in, 16 + covered - 32);
        w.out[3] = crc;
        for b in sb.iter().take(blocks) {
            for ch in 0..chs {
                for s in 0..m {
                    let n = bits[ch][s];
                    if n == 0 { continue; }
                    let levels = (1i64 << n) - 1;
                    let shift = scale[ch][s] as u32 + 16;
                    let q = ((b[ch][s] + (1i64 << shift)) * levels) >> (shift + 1);
                    w.put(q.clamp(0, levels - 1) as u32, n);
                }
            }
        }
        len
    }
}

/// A sine for each ear, to hear a link before any program feeds it: 256 steps of a period, scaled.
pub struct Tone { phase: [u32; 2], step: [u32; 2], pub level: i16 }

const SINE: [i16; 256] = [
    0, 804, 1608, 2410, 3212, 4011, 4808, 5602, 6393, 7179, 7962, 8739, 9512, 10278, 11039, 11793,
    12539, 13279, 14010, 14732, 15446, 16151, 16846, 17530, 18204, 18868, 19519, 20159, 20787, 21403, 22005, 22594,
    23170, 23731, 24279, 24811, 25329, 25832, 26319, 26790, 27245, 27683, 28105, 28510, 28898, 29268, 29621, 29956,
    30273, 30571, 30852, 31113, 31356, 31580, 31785, 31971, 32137, 32285, 32412, 32521, 32609, 32678, 32728, 32757,
    32767, 32757, 32728, 32678, 32609, 32521, 32412, 32285, 32137, 31971, 31785, 31580, 31356, 31113, 30852, 30571,
    30273, 29956, 29621, 29268, 28898, 28510, 28105, 27683, 27245, 26790, 26319, 25832, 25329, 24811, 24279, 23731,
    23170, 22594, 22005, 21403, 20787, 20159, 19519, 18868, 18204, 17530, 16846, 16151, 15446, 14732, 14010, 13279,
    12539, 11793, 11039, 10278, 9512, 8739, 7962, 7179, 6393, 5602, 4808, 4011, 3212, 2410, 1608, 804,
    0, -804, -1608, -2410, -3212, -4011, -4808, -5602, -6393, -7179, -7962, -8739, -9512, -10278, -11039, -11793,
    -12539, -13279, -14010, -14732, -15446, -16151, -16846, -17530, -18204, -18868, -19519, -20159, -20787, -21403, -22005, -22594,
    -23170, -23731, -24279, -24811, -25329, -25832, -26319, -26790, -27245, -27683, -28105, -28510, -28898, -29268, -29621, -29956,
    -30273, -30571, -30852, -31113, -31356, -31580, -31785, -31971, -32137, -32285, -32412, -32521, -32609, -32678, -32728, -32757,
    -32767, -32757, -32728, -32678, -32609, -32521, -32412, -32285, -32137, -31971, -31785, -31580, -31356, -31113, -30852, -30571,
    -30273, -29956, -29621, -29268, -28898, -28510, -28105, -27683, -27245, -26790, -26319, -25832, -25329, -24811, -24279, -23731,
    -23170, -22594, -22005, -21403, -20787, -20159, -19519, -18868, -18204, -17530, -16846, -16151, -15446, -14732, -14010, -13279,
    -12539, -11793, -11039, -10278, -9512, -8739, -7962, -7179, -6393, -5602, -4808, -4011, -3212, -2410, -1608, -804,
];

impl Tone {
    /// `left` and `right` hertz at `rate` hertz, at `level` (of 32767).
    pub fn new(left: u32, right: u32, rate: u32, level: i16) -> Self {
        let step = |hz: u32| ((hz as u64) << 32).checked_div(rate as u64).unwrap_or(0) as u32;
        Self { phase: [0; 2], step: [step(left), step(right)], level }
    }

    /// Interleaved samples for `channels` channels into `out`.
    pub fn fill(&mut self, out: &mut [i16], channels: usize) {
        for frame in out.chunks_mut(channels) {
            for (ch, v) in frame.iter_mut().enumerate() {
                let c = ch.min(1);
                *v = ((SINE[(self.phase[c] >> 24) as usize] as i32 * self.level as i32) >> 15) as i16;
            }
            for c in 0..2 { self.phase[c] = self.phase[c].wrapping_add(self.step[c]); }
        }
    }
}
