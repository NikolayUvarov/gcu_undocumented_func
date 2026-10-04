// Formant synthesizer: 5 ms frames with target interpolation, F1–F5 cascade for voicing and aspiration,
// parallel resonator for frication; intonation falls across a phrase and rises at the end of a question.
use crate::dsp::{Antiresonator, Glottis, Noise, Resonator, RATE};
use crate::phonemes::{segments, Ph, Segment, Target, Unit, NASAL_POLE};

pub const FRAME: usize = 80; // 5 ms at 16 kHz
/// Samples without excitation after which the filters are cleared (30 ms): their fixed-point feedback can keep a small
/// state circulating as a limit cycle — a quiet tone of about −55 dBFS that lasted through every pause and to the end
/// of the phrase (issue 087). By then the ringing of the last sound has decayed below −60 dBFS. Clearing at that
/// point, rather than rounding the feedback differently, changes the speech only by the cycle no longer carried into
/// the next sound (about 40 dB below the signal); rounding toward zero changed it by 26 dB and still left cycles.
const SETTLE: usize = 480;

#[derive(Clone, Copy)]
pub struct Voice { pub pitch: i64, pub rate: i64 }
impl Default for Voice { fn default() -> Self { Self { pitch: 112, rate: 100 } } }

struct State { cascade: [Resonator; 5], fric: Resonator, nasal_pole: Resonator, nasal_zero: Antiresonator, glottis: Glottis, noise: Noise, current: Target, dc_x: i64, dc_y: i64, phrase_ms: i64, tilt: i64, last_noise: i64, toward: Option<[i32; 3]>, attack: Option<i64>, jitter: i64, quiet: usize }

fn lerp(a: i32, b: i32, k: i64) -> i32 { a + ((b - a) as i64 * k >> 10) as i32 }

impl State {
    fn new() -> Self {
        let mut cascade = [Resonator::default(); 5];
        cascade[3].set(3300, 250, false); cascade[4].set(3850, 300, false);
        let mut nasal_pole = Resonator::default(); nasal_pole.set(NASAL_POLE, 100, false);
        Self { cascade, fric: Resonator::default(), nasal_pole, nasal_zero: Antiresonator::default(), glottis: Glottis::new(), noise: Noise::new(), current: Target { f: [500, 1500, 2500], b: [70, 90, 150], nz: NASAL_POLE, ..Target::default() }, dc_x: 0, dc_y: 0, phrase_ms: 0, tilt: 0, last_noise: 0, toward: None, attack: None, jitter: 0, quiet: 0 }
    }

    // Silence after SETTLE samples without excitation: every filter's state is cleared.
    fn settle(&mut self) {
        for resonator in self.cascade.iter_mut() { resonator.clear(); }
        self.fric.clear(); self.nasal_pole.clear(); self.nasal_zero.clear();
        self.tilt = 0; self.dc_x = 0; self.dc_y = 0;
    }

    // Segment with a three-point pitch contour (rise on stress).
    fn render_contour(&mut self, segment: &Segment, voice: Voice, points: &[i64; 3], sink: &mut dyn FnMut(&[i16])) {
        let mut first = *segment; first.ms = segment.ms / 2; let mut second = *segment; second.ms = segment.ms - first.ms;
        second.blend = (segment.blend - first.ms).max(1);
        let toward = self.toward.take();
        self.render(&first, voice, (points[0], points[1]), sink);
        self.toward = toward;
        self.render(&second, voice, (points[1], points[2]), sink);
    }

    // One segment: interpolate from the current parameters to the target, synthesize samples frame by frame into `sink`.
    fn render(&mut self, segment: &Segment, voice: Voice, contour: (i64, i64), sink: &mut dyn FnMut(&[i16])) {
        let samples = (segment.ms as i64 * RATE / 1000 * 100 / voice.rate.clamp(50, 200)) as usize;
        let blend = (segment.blend as i64 * RATE / 1000).max(1); let amp_blend = if segment.snap { 1 } else { self.attack.take().unwrap_or(8) * RATE / 1000 };
        let start = self.current; let mut buffer = [0i16; FRAME]; let mut done = 0usize;
        while done < samples {
            let k = ((done as i64) << 10) / blend; let ka = ((done as i64) << 10) / amp_blend;
            let (k, ka) = (k.min(1024), ka.min(1024));
            // Pauses change only amplitudes: formants stay put (otherwise the resonators break up and click).
            let silent = segment.t.f[0] == 0; let t = &segment.t; let mut cur = start;
            if !silent { for i in 0..3 { cur.f[i] = lerp(start.f[i], t.f[i], k); cur.b[i] = lerp(start.b[i], t.b[i], k); } cur.nz = lerp(start.nz.max(NASAL_POLE), t.nz.max(NASAL_POLE), k); }
            cur.av = lerp(start.av, t.av, ka); cur.ah = lerp(start.ah, t.ah, ka); cur.af = lerp(start.af, t.af, ka);
            cur.ff = if t.af > 0 { t.ff } else { start.ff }; cur.fb = if t.af > 0 { t.fb } else { start.fb };
            // Vowel offglide: in the last 40 ms formants move toward the next consonant's locus (a place-of-articulation cue).
            if let Some(next) = self.toward {
                let out = (40 * RATE / 1000) as usize;
                if done + out > samples { let p = (((done + out - samples) as i64) << 10) / out as i64 * 6 / 10; for i in 0..3 { cur.f[i] = lerp(cur.f[i], next[i], p); } }
            }
            if cur.f[0] == 0 { cur.f = [500, 1500, 2500]; cur.b = [70, 90, 150]; } // silence at the start: neutral formants
            for i in 0..3 { self.cascade[i].set(cur.f[i], cur.b[i], false); }
            self.nasal_zero.set(cur.nz.max(NASAL_POLE), 100);
            if cur.ff > 0 { self.fric.set(cur.ff, cur.fb, true); }
            // Pitch within a segment: linear from contour.0 to contour.1.
            let f0 = contour.0 + (contour.1 - contour.0) * done as i64 / samples.max(1) as i64;
            let count = FRAME.min(samples - done);
            let excited = cur.av != 0 || cur.ah != 0 || cur.af != 0;
            for sample in buffer[..count].iter_mut() {
                if excited { self.quiet = 0; } else { self.quiet += 1; if self.quiet == SETTLE { self.settle(); } }
                // Slight pitch jitter and breath noise in the open phase: a voice is not perfectly periodic.
                let (pulse, period) = self.glottis.next(f0 + self.jitter);
                if period { self.jitter = (self.noise.next() * f0 / 4096) / 100; }
                let noise = self.noise.next();
                // High-frequency-boosted noise (first difference): otherwise aspiration and wide frication resonators boom in the lows.
                let tilted = noise - self.last_noise; self.last_noise = noise;
                // Source spectral tilt: a one-pole filter softens the "buzz" of the upper harmonics.
                self.tilt = (pulse * cur.av as i64 / 1000 + self.tilt * 3) / 4;
                let breath = if pulse > 0 { tilted * cur.av as i64 / 1000 / 12 } else { 0 };
                let mut y = self.nasal_zero.run(self.nasal_pole.run(self.tilt * 2 + breath + tilted * cur.ah as i64 / 1000));
                for resonator in self.cascade.iter_mut() { y = resonator.run(y); }
                let fric = if cur.af > 0 { self.fric.run(tilted) * cur.af as i64 / 1000 } else { self.fric.run(0) };
                let mixed = y * 2 + fric * 3;
                let dc = mixed - self.dc_x + (self.dc_y * 4064 >> 12); self.dc_x = mixed; self.dc_y = dc; // DC-blocking filter
                *sample = (dc * 2 / 3).clamp(-32_000, 32_000) as i16; // headroom: no clipping on open vowels
            }
            sink(&buffer[..count]);
            done += count;
            self.current = cur;
        }
        self.phrase_ms += segment.ms as i64;
    }
}

/// Speaks phonemes; `sink` receives 16 kHz mono in chunks of up to 80 samples.
pub fn speak(units: &[Unit], voice: Voice, sink: &mut dyn FnMut(&[i16])) { speak_labeled(units, voice, &mut |chunk, _| sink(chunk)); }

/// `speak`, telling for every chunk which unit it belongs to (its index in `units`): the phone labels the voice
/// recognizer is trained with (scripts/voice_train.rs).
pub fn speak_labeled(units: &[Unit], voice: Voice, sink: &mut dyn FnMut(&[i16], usize)) {
    let mut state = State::new();
    let base = voice.pitch.clamp(60, 300);
    for (index, unit) in units.iter().enumerate() {
        let sink = &mut |chunk: &[i16]| sink(chunk, index);
        let rest = &units[index + 1..];
        let back = rest.iter().find(|u| u.ph.vowel()).is_some_and(|u| matches!(u.ph, Ph::A | Ph::O | Ph::U | Ph::Y | Ph::Uh | Ph::Ah));
        // The last vowel of a phrase is lengthened and carries the final intonation.
        let phrase_end = rest.iter().take_while(|u| !u.ph.vowel()).find_map(|u| match u.ph { Ph::End(kind) => Some(kind), Ph::Pause(ms) if ms > crate::text::WORD_GAP_MS => Some(b','), _ => None });
        let final_vowel = unit.ph.vowel() && phrase_end.is_some();
        let mut parts = [Segment { t: Target::default(), ms: 0, blend: 0, snap: false }; 4];
        let count = segments(*unit, back, final_vowel, &mut parts);
        if unit.ph.vowel() && index > 0 && matches!(units[index - 1].ph, Ph::M | Ph::N) { parts[0].blend = 15; }
        // After a soft consonant the transition is shorter: a long F2 rise sounds like an extra [й].
        if unit.ph.vowel() && index > 0 && (units[index - 1].soft || units[index - 1].ph == Ph::Ch) { parts[0].blend = 25; }
        // Locus of the next consonant (with no pause in between) for the vowel offglide.
        let next = rest.first().filter(|u| unit.ph.vowel() && !u.ph.vowel() && !matches!(u.ph, Ph::Pause(_) | Ph::End(_)));
        let toward = next.map(|u| { let mut first = [parts[0]; 4]; segments(*u, back, false, &mut first); first[0].t.f });
        // After a pause, vowels and sonorants ramp up smoothly: a sharp attack sounds like a plosive.
        let after_pause = index == 0 || matches!(units[index - 1].ph, Ph::End(_)) || matches!(units[index - 1].ph, Ph::Pause(ms) if ms > 0);
        state.attack = (after_pause && (unit.ph.vowel() || matches!(unit.ph, Ph::J | Ph::W | Ph::L | Ph::R | Ph::M | Ph::N | Ph::V))).then_some(25);
        for (part_index, part) in parts[..count].iter().enumerate() {
            state.toward = if part_index + 1 == count { toward } else { None };
            let decline = |ms: i64| base * (114 - 22 * ms.min(2400) / 2400) / 100;
            let start = decline(state.phrase_ms); let mut end = decline(state.phrase_ms + part.ms as i64);
            // Stressed vowel: pitch rise; last vowel of the phrase: final intonation.
            if unit.stress && !final_vowel { let peak = start * 118 / 100; state.render_contour(part, voice, &[start, peak, end * 104 / 100], sink); continue; }
            if final_vowel { end = match phrase_end { Some(b'?') => base * 140 / 100, Some(b',') => base * 105 / 100, _ => base * 78 / 100 }; }
            state.render(part, voice, (start, end), sink);
        }
        if matches!(unit.ph, Ph::End(_)) { state.phrase_ms = 0; }
    }
}
