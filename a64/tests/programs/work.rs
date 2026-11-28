// The same work computed natively by the host test and, built for aarch64-unknown-none-softfloat, on the emulator
// (215-PRT-0016): each slot of `out` must agree. Only core, and wrapping arithmetic, so both builds mean the same.
use core::fmt::Write;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

pub const SLOTS: usize = 16;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
}

fn mix(h: u64, v: u64) -> u64 { (h ^ v).wrapping_mul(0x0000_0100_0000_01B3).rotate_left(29) }

fn sha256(data: &[u8]) -> [u32; 8] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut block = [0u8; 64];
    let total = data.len() + 9;
    let blocks = (total + 63) / 64;
    for b in 0..blocks {
        for (i, byte) in block.iter_mut().enumerate() {
            let at = b * 64 + i;
            *byte = if at < data.len() {
                data[at]
            } else if at == data.len() {
                0x80
            } else if at >= blocks * 64 - 8 {
                ((data.len() as u64 * 8) >> (8 * (blocks * 64 - 1 - at))) as u8
            } else {
                0
            };
        }
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]); }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut s = h;
        for i in 0..64 {
            let t1 = s[7]
                .wrapping_add(s[4].rotate_right(6) ^ s[4].rotate_right(11) ^ s[4].rotate_right(25))
                .wrapping_add((s[4] & s[5]) ^ (!s[4] & s[6]))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let t2 = (s[0].rotate_right(2) ^ s[0].rotate_right(13) ^ s[0].rotate_right(22))
                .wrapping_add((s[0] & s[1]) ^ (s[0] & s[2]) ^ (s[1] & s[2]));
            s = [t1.wrapping_add(t2), s[0], s[1], s[2], s[3].wrapping_add(t1), s[4], s[5], s[6]];
        }
        for i in 0..8 { h[i] = h[i].wrapping_add(s[i]); }
    }
    h
}

struct Text { bytes: [u8; 512], len: usize }
impl Write for Text {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if self.len == self.bytes.len() { return Err(core::fmt::Error); }
            self.bytes[self.len] = b;
            self.len += 1;
        }
        Ok(())
    }
}

#[inline(never)]
pub fn work(out: &mut [u64; SLOTS]) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut data = [0u8; 4096];
    for b in data.iter_mut() { *b = rng.next() as u8; }

    // 0, 1: SHA-256 of 4096 and of 55 bytes (rotates, shifts, adds, byte loads).
    let d = sha256(&data);
    out[0] = d.iter().fold(0, |h, &v| mix(h, v as u64));
    out[1] = sha256(&data[..55]).iter().fold(0, |h, &v| mix(h, v as u64));

    // 2: division and remainder, signed and unsigned, 64 and 32 bits, the edges included.
    let mut h = 0;
    for i in 0..2000u64 {
        let (a, b) = (rng.next(), rng.next() >> (i % 61));
        let b = if i % 97 == 0 { 0 } else { b | 1 };
        if b != 0 {
            h = mix(h, a / b);
            h = mix(h, a % b);
            h = mix(h, (a as i64).wrapping_div(b as i64) as u64);
            h = mix(h, (a as i64).wrapping_rem(b as i64) as u64);
            h = mix(h, ((a as u32) / (b as u32 | 1)) as u64);
            h = mix(h, ((a as i32).wrapping_div(b as i32 | 1)) as u64);
        }
    }
    h = mix(h, i64::MIN.wrapping_div(-1) as u64);
    h = mix(h, i32::MIN.wrapping_rem(-1) as u64);
    out[2] = h;

    // 3: 128-bit products (UMULH, SMULH, MADD) and a 256-limb multiplication with carries.
    let mut h = 0;
    for _ in 0..1000 {
        let (a, b) = (rng.next(), rng.next());
        h = mix(h, ((a as u128 * b as u128) >> 64) as u64);
        h = mix(h, ((a as i64 as i128 * b as i64 as i128) >> 64) as u64);
        h = mix(h, (a as u128).wrapping_mul(b as u128) as u64);
    }
    let x: [u64; 8] = core::array::from_fn(|_| rng.next());
    let y: [u64; 8] = core::array::from_fn(|_| rng.next());
    let mut z = [0u64; 16];
    for i in 0..8 {
        let mut carry = 0u128;
        for j in 0..8 {
            let t = x[i] as u128 * y[j] as u128 + z[i + j] as u128 + carry;
            z[i + j] = t as u64;
            carry = t >> 64;
        }
        z[i + 8] = carry as u64;
    }
    out[3] = z.iter().fold(h, |h, &v| mix(h, v));

    // 4: sorting (loads, stores, compares, branches).
    let mut v: [u32; 300] = core::array::from_fn(|_| rng.next() as u32);
    v.sort_unstable();
    let mut w: [i16; 300] = core::array::from_fn(|_| rng.next() as i16);
    w.sort_unstable();
    out[4] = v.iter().map(|&x| x as u64).chain(w.iter().map(|&x| x as u64)).fold(0, mix);

    // 5: bit operations.
    let mut h = 0;
    for i in 0..1000u32 {
        let a = rng.next() >> (i % 64);
        h = mix(h, a.leading_zeros() as u64 | (a.trailing_zeros() as u64) << 8 | (a.count_ones() as u64) << 16);
        h = mix(h, a.rotate_left(i % 64) ^ a.swap_bytes() ^ a.reverse_bits());
        let b = a as u32;
        h = mix(h, (b.leading_zeros() | b.trailing_zeros() << 8) as u64 ^ b.rotate_right(i % 32) as u64 ^ b.swap_bytes() as u64);
        h = mix(h, (a as i64 >> (i % 64)) as u64 ^ ((b as i32) >> (i % 32)) as u64);
        h = mix(h, (a as u16).swap_bytes() as u64 ^ ((a as i8) as i64) as u64 ^ ((a as i16) as i64) as u64);
    }
    out[5] = h;

    // 6: overflowing, checked and saturating arithmetic (the flags).
    let mut h = 0;
    for _ in 0..1000 {
        let (a, b) = (rng.next(), rng.next());
        let (r, o) = (a as i64).overflowing_add(b as i64);
        h = mix(h, r as u64 ^ o as u64);
        let (r, o) = (a as i32).overflowing_sub(b as i32);
        h = mix(h, r as u64 ^ (o as u64) << 40);
        let (r, o) = a.overflowing_mul(b >> 32);
        h = mix(h, r ^ o as u64);
        h = mix(h, (a as i64).saturating_mul(b as i64 >> 33) as u64);
        h = mix(h, (a as u32).saturating_add(b as u32) as u64 ^ (a as u8).saturating_sub(b as u8) as u64);
        h = mix(h, (a as i64).checked_add(b as i64).map_or(1, |v| v as u64));
        h = mix(h, ((a as i32) < (b as i32)) as u64 | (((a as u32) < (b as u32)) as u64) << 1 | ((a < b) as u64) << 2);
    }
    out[6] = h;

    // 7: soft-float: the aarch64 build computes in software what the host's hardware does.
    let mut h = 0;
    for _ in 0..500 {
        let (a, b) = (rng.next(), rng.next());
        let (x, y) = ((a as i64 as f64) / 3.0, (b >> 11) as f64 * 1.0e-7);
        h = mix(h, (x + y).to_bits() ^ (x * y).to_bits() ^ (x / (y + 1.0)).to_bits() ^ (x - y).to_bits());
        let (f, g) = (a as u32 as f32 * 0.5, (b as i32) as f32);
        h = mix(h, ((f * g).to_bits() ^ (f + g).to_bits() ^ (f / (g + 0.25)).to_bits()) as u64);
        h = mix(h, (x as i32) as u64 ^ (y as u64) ^ ((f as f64).to_bits()) ^ ((x as f32).to_bits() as u64));
        h = mix(h, (x < y) as u64 | ((f >= g) as u64) << 1 | ((x == x) as u64) << 2);
    }
    out[7] = h;

    // 8: formatting through core::fmt.
    let mut text = Text { bytes: [0; 512], len: 0 };
    for _ in 0..12 {
        let a = rng.next();
        let _ = write!(text, "{} {:x} {:>8} {:+} {:o}|", a, a >> 7, a as u16, a as i32, a as u8);
    }
    let _ = write!(text, "{:.3} {:e}", 1.0 / 3.0, 123456.789f64);
    out[8] = text.bytes[..text.len].iter().fold(text.len as u64, |h, &b| mix(h, b as u64));

    // 9: atomics, by exclusive loads and stores.
    let counter = AtomicU64::new(5);
    let flags = AtomicU8::new(0);
    for i in 0..100u64 {
        counter.fetch_add(i, Ordering::SeqCst);
        let _ = counter.compare_exchange(i * 3, 7, Ordering::AcqRel, Ordering::Acquire);
        flags.fetch_or((i % 8) as u8, Ordering::Relaxed);
        flags.fetch_xor(1, Ordering::Relaxed);
    }
    out[9] = mix(counter.swap(1, Ordering::SeqCst), flags.load(Ordering::SeqCst) as u64);

    // 10: copies and moves at odd offsets, and comparisons of slices.
    let mut buffer = [0u8; 600];
    buffer[..512].copy_from_slice(&data[..512]);
    buffer.copy_within(3..400, 17);
    buffer.copy_within(101..500, 5);
    let same = (buffer[..64] == data[..64]) as u64;
    out[10] = buffer.iter().fold(same, |h, &b| mix(h, b as u64));

    // 11: sign- and zero-extending loads of bytes and halfwords.
    let mut h = 0i64;
    for i in 0..4000 {
        h = h.wrapping_add(data[i] as i8 as i64).wrapping_mul(3);
        if i % 2 == 0 { h ^= i16::from_le_bytes([data[i], data[i + 1]]) as i64; }
        if i % 4 == 0 { h = h.wrapping_add(u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as i32 as i64); }
    }
    out[11] = h as u64;

    // 12: a byte state machine and table lookups (jump tables, indexed loads).
    let mut state = 0u8;
    let mut counts = [0u32; 7];
    for &b in data.iter() {
        state = match (state, b % 5) {
            (0, 0) | (3, 1) => 1,
            (1, x) if x > 2 => 2,
            (2, _) => (b % 7) as u8,
            (4, 4) => 6,
            (s, x) => (s + x) % 7,
        };
        counts[state as usize] = counts[state as usize].wrapping_add(b as u32);
    }
    out[12] = counts.iter().fold(state as u64, |h, &c| mix(h, c as u64));

    // 13: u8, u16, u32 arithmetic that wraps at its own width.
    let (mut a8, mut a16, mut a32) = (1u8, 1u16, 1u32);
    for &b in data.iter() {
        a8 = a8.wrapping_mul(b | 1).wrapping_add(b >> 1);
        a16 = a16.wrapping_mul(b as u16 | 1) ^ (a16 >> 3);
        a32 = a32.wrapping_mul(0x01000193) ^ b as u32;
    }
    out[13] = a8 as u64 | (a16 as u64) << 8 | (a32 as u64) << 24;

    // 14: 128-bit division (a library call made of shifts and subtractions).
    let mut h = 0;
    for _ in 0..200 {
        let (a, b) = ((rng.next() as u128) << 64 | rng.next() as u128, (rng.next() >> 20) as u128 | 1);
        h = mix(h, (a / b) as u64 ^ (a % b) as u64 ^ ((a as i128) / (b as i128 + 3)) as u64);
    }
    out[14] = h;

    // 15: how far the generator went, so a skipped loop shows.
    out[15] = rng.next();
}
