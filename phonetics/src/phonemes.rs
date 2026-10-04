// Phonemes and their acoustic targets for formant synthesis (male voice, 16 kHz).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ph { A, O, U, E, I, Y, Schwa, Ae, Ih, Uh, Ah, Er, J, W, L, R, M, N, P, B, T, D, K, G, F, V, S, Z, Sh, Zh, Shch, X, H, Ts, Ch, Th, Dh, Pause(u16), End(u8) }

#[derive(Clone, Copy, Debug)]
pub struct Unit { pub ph: Ph, pub soft: bool, pub stress: bool }

/// The recognizer's phone alphabet (`mind::voice`): one name per phoneme, soft and hard consonants alike; pauses and
/// the end of a phrase are silence, which has no name here.
pub const PHONES: [&str; 37] = ["a", "o", "u", "e", "i", "y", "@", "&", "I", "U", "^", "3", "j", "w", "l", "r", "m", "n", "p", "b", "t", "d", "k", "g",
                                "f", "v", "s", "z", "sh", "zh", "shch", "x", "h", "ts", "ch", "th", "dh"];

impl Ph {
    /// This phoneme's name in `PHONES`; None for a pause or the end of a phrase.
    pub fn name(self) -> Option<&'static str> {
        let index = match self {
            Ph::A => 0, Ph::O => 1, Ph::U => 2, Ph::E => 3, Ph::I => 4, Ph::Y => 5, Ph::Schwa => 6, Ph::Ae => 7, Ph::Ih => 8, Ph::Uh => 9, Ph::Ah => 10, Ph::Er => 11,
            Ph::J => 12, Ph::W => 13, Ph::L => 14, Ph::R => 15, Ph::M => 16, Ph::N => 17, Ph::P => 18, Ph::B => 19, Ph::T => 20, Ph::D => 21, Ph::K => 22, Ph::G => 23,
            Ph::F => 24, Ph::V => 25, Ph::S => 26, Ph::Z => 27, Ph::Sh => 28, Ph::Zh => 29, Ph::Shch => 30, Ph::X => 31, Ph::H => 32, Ph::Ts => 33, Ph::Ch => 34,
            Ph::Th => 35, Ph::Dh => 36, Ph::Pause(_) | Ph::End(_) => return None,
        };
        Some(PHONES[index])
    }
}

impl Ph {
    pub fn vowel(self) -> bool { matches!(self, Ph::A | Ph::O | Ph::U | Ph::E | Ph::I | Ph::Y | Ph::Schwa | Ph::Ae | Ph::Ih | Ph::Uh | Ph::Ah | Ph::Er) }
    pub fn voiceless(self) -> bool { matches!(self, Ph::P | Ph::T | Ph::K | Ph::F | Ph::S | Ph::Sh | Ph::Shch | Ph::X | Ph::H | Ph::Ts | Ph::Ch | Ph::Th) }
    pub fn obstruent(self) -> bool { self.voiceless() || matches!(self, Ph::B | Ph::D | Ph::G | Ph::V | Ph::Z | Ph::Zh | Ph::Dh) }
    pub fn devoiced(self) -> Ph { match self { Ph::B => Ph::P, Ph::D => Ph::T, Ph::G => Ph::K, Ph::V => Ph::F, Ph::Z => Ph::S, Ph::Zh => Ph::Sh, Ph::Dh => Ph::Th, other => other } }
    pub fn voiced(self) -> Ph { match self { Ph::P => Ph::B, Ph::T => Ph::D, Ph::K => Ph::G, Ph::S => Ph::Z, Ph::Sh => Ph::Zh, other => other } }
}

// Segment target: formants F1–F3 and bandwidths, amplitudes of voicing (av), aspiration (ah) and frication (af) with its frequency.
#[derive(Clone, Copy, Default, Debug)]
pub struct Target { pub f: [i32; 3], pub b: [i32; 3], pub av: i32, pub ah: i32, pub af: i32, pub ff: i32, pub fb: i32, pub nz: i32 }

#[derive(Clone, Copy, Debug)]
pub struct Segment { pub t: Target, pub ms: i32, pub blend: i32, pub snap: bool }

const fn formants(f1: i32, f2: i32, f3: i32) -> Target { Target { f: [f1, f2, f3], b: [70, 90, 150], av: 0, ah: 0, af: 0, ff: 0, fb: 0, nz: NASAL_POLE } }

// Nasal pole; a zero at the same frequency cancels it (oral sounds), a shifted zero gives nasal timbre.
pub const NASAL_POLE: i32 = 270;

fn vowel_target(ph: Ph) -> Target {
    match ph {
        Ph::A => formants(720, 1240, 2550), Ph::O => formants(520, 860, 2450), Ph::U => formants(340, 720, 2300),
        Ph::E => formants(490, 1820, 2550), Ph::I => formants(290, 2250, 2950), Ph::Y => formants(320, 1500, 2350),
        Ph::Ae => formants(660, 1720, 2450),
        // English [ɪ ʊ ʌ ɝ]: lax vowels and an r-colored one with low F3.
        Ph::Ih => formants(400, 1950, 2550), Ph::Uh => formants(440, 1050, 2250), Ph::Ah => formants(630, 1200, 2400),
        Ph::Er => formants(470, 1380, 1700), _ => formants(500, 1450, 2500),
    }
}

// Consonant F2 locus by place of articulation; soft consonants pull formants toward [i].
fn locus(ph: Ph, soft: bool, back: bool) -> Target {
    if soft { return formants(270, 1950, 2800); }
    match ph {
        Ph::P | Ph::B | Ph::M | Ph::F | Ph::V | Ph::W => formants(260, 850, 2250),
        Ph::K | Ph::G | Ph::X => if back { formants(280, 1250, 2300) } else { formants(270, 2100, 2750) },
        Ph::Sh | Ph::Zh | Ph::Ch | Ph::Shch => formants(300, 1750, 2350),
        _ => formants(270, 1650, 2600),
    }
}

/// Segments of one phoneme; `back` means the next vowel is a back vowel (for [k g x]); durations at 100 % tempo.
pub fn segments(unit: Unit, back: bool, final_vowel: bool, out: &mut [Segment; 4]) -> usize {
    let Unit { ph, soft, stress } = unit;
    let seg = |t: Target, ms: i32, blend: i32| Segment { t, ms, blend, snap: false };
    let mut l = locus(ph, soft, back);
    let fric = |mut t: Target, af: i32, ff: i32, fb: i32, av: i32| { t.af = af; t.ff = ff; t.fb = fb; t.av = av; t };
    let mut n = 0;
    let mut push = |s: Segment| { out[n] = s; n += 1; };
    match ph {
        _ if ph.vowel() => {
            let mut t = vowel_target(ph); t.av = 1000;
            let ms = if ph == Ph::Schwa { 45 } else { 50 } + if stress { 70 } else { 0 } + if final_vowel { 50 } else { 0 };
            push(Segment { t, ms, blend: 45, snap: false });
        }
        Ph::J => { let mut t = formants(270, 2250, 3000); t.av = 800; push(seg(t, 50, 30)); }
        Ph::W => { let mut t = formants(300, 650, 2250); t.av = 800; push(seg(t, 50, 30)); }
        Ph::L => { let mut t = if soft { formants(300, 1900, 2800) } else { formants(360, 1000, 2650) }; t.av = 750; t.b = [90, 150, 250]; push(seg(t, 60, 30)); }
        Ph::R => {
            // Single-contact trill (tap): a short closure in the middle.
            let mut t = if soft { formants(380, 1800, 2400) } else { formants(420, 1250, 1650) }; t.av = 800;
            push(seg(t, 18, 25)); let mut tap = t; tap.av = 150; push(seg(tap, 14, 6)); push(seg(t, 16, 6));
        }
        Ph::M | Ph::N => {
            let mut t = if ph == Ph::M { formants(250, 1000, 2200) } else if soft { formants(250, 2000, 2800) } else { formants(250, 1650, 2550) };
            // Nasal murmur: strong low resonance, blurred upper formants, abrupt transition into the vowel.
            t.av = 750; t.b = [60, 350, 500]; t.nz = if ph == Ph::M { 1000 } else { 1500 }; push(Segment { t, ms: 65, blend: 12, snap: true });
        }
        Ph::Dh => push(seg(fric(l, 120, 5000, 4000, 550), 50, 25)), // English voiced [ð]
        Ph::F | Ph::Th => push(seg(fric(l, 260, if ph == Ph::F { 3500 } else { 5500 }, 5000, 0), 95, 30)),
        Ph::V => { l = formants(300, 1300, 2300); push(seg(fric(l, 40, 3500, 2500, 700), 60, 25)); }
        Ph::S => push(seg(fric(l, 850, 5600, 1600, 0), 100, 30)),
        Ph::Z => push(seg(fric(l, 450, 5600, 1600, 450), 80, 30)),
        Ph::Sh => push(seg(fric(l, 750, 2500, 1300, 0), 105, 30)),
        Ph::Zh => push(seg(fric(l, 380, 2500, 1300, 450), 80, 30)),
        Ph::Shch => push(seg(fric(formants(280, 2100, 2900), 700, 3300, 1500, 0), 150, 30)),
        Ph::X => push(seg(fric(l, 500, 1900, 1400, 0), 95, 30)),
        Ph::H => { let mut t = formants(500, 1450, 2500); t.ah = 450; push(seg(t, 60, 20)); }
        Ph::P | Ph::B | Ph::T | Ph::D | Ph::K | Ph::G | Ph::Ts | Ph::Ch => {
            let voiced = matches!(ph, Ph::B | Ph::D | Ph::G);
            let (ff, fb, af) = match ph { Ph::P | Ph::B => (1200, 2000, 500), Ph::K | Ph::G => (if back { 1500 } else { 2500 }, 700, 900), _ => (4200, 2500, 850) };
            // Closure (with a voice bar for voiced stops), burst, and short aspiration for voiceless ones.
            let mut closure = l; closure.av = if voiced { 260 } else { 0 }; closure.f[0] = 200; push(seg(closure, if voiced { 50 } else { 58 }, 25));
            let mut burst = fric(l, if voiced { af * 2 / 3 } else { af }, ff, fb, if voiced { 300 } else { 0 }); burst.f[0] = 250;
            push(Segment { t: burst, ms: if ph == Ph::K || ph == Ph::G { 20 } else { 10 }, blend: 2, snap: true });
            match ph {
                Ph::Ts => push(seg(fric(l, 800, 5600, 1600, 0), 70, 10)),
                Ph::Ch => push(seg(fric(formants(280, 2100, 2900), 650, 3200, 1600, 0), 65, 10)),
                _ if !voiced => { let mut aspiration = l; aspiration.ah = 280; push(seg(aspiration, if ph == Ph::K { 25 } else { 15 }, 4)); }
                _ => {}
            }
        }
        Ph::Pause(ms) => push(seg(Target::default(), ms as i32, 10)),
        Ph::End(_) => push(seg(Target::default(), 320, 10)),
        _ => {}
    }
    n
}
