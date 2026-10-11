// The order of an isochronous ring of iTDs (158), apart from the registers: frame list entry j names iTD j % frames.
// Each look collects every iTD the controller finished, in the order of the frames they ran in, and arms every free one
// that is at least two frames ahead of the controller (it may hold an iTD that long). An iTD remembers the frame it was
// armed for, counted past the frame list's wrap, so its packets come in time order however late the look.
// tests/iso_ring_host.rs runs it against a model of the controller.

/// What the ring needs of the descriptors: whether an iTD still has a transaction to run, and to collect or arm one.
pub trait Descriptors {
    fn active(&self, k: usize) -> bool;
    fn collect(&mut self, k: usize);
    fn arm(&mut self, k: usize);
}

pub const MAX_FRAMES: usize = 64;
const LIST: u64 = 1024; // frame list entries: FRINDEX's frame wraps here

/// `late` counts the runs the controller found an iTD unarmed (frames lost) since the first arming.
pub struct Ring { pub frames: usize, pub armed: u64, pub late: u64, now: u64, last: usize, due: [u64; MAX_FRAMES] }

impl Ring {
    /// A ring of `frames` iTDs (a power of two up to 64, dividing 1024), the controller in frame list entry `frame`.
    pub fn new(frames: usize, frame: usize) -> Self {
        // As if each iTD had last run in the frame it had before this one (those of this frame and the next: in them).
        let (n, now) = (frames as u64, frame as u64 + LIST);
        let mut due = [0; MAX_FRAMES];
        for (k, d) in due.iter_mut().enumerate().take(frames) { let ahead = (k as u64 + n - now % n) % n; *d = now + ahead - if ahead < 2 { 0 } else { n }; }
        Self { frames, armed: 0, late: 0, now, last: frame, due }
    }

    /// One look, the controller in frame list entry `frame` (looks come less than 1024 frames apart).
    pub fn pump(&mut self, frame: usize, itds: &mut impl Descriptors) {
        self.now += (frame as u64 + LIST - self.last as u64) % LIST;
        self.last = frame;
        // The finished ones, oldest first.
        let mut done = [0usize; MAX_FRAMES];
        let mut count = 0;
        for k in (0..self.frames).filter(|&k| self.armed & 1 << k != 0 && !itds.active(k)) { done[count] = k; count += 1; }
        done[..count].sort_unstable_by_key(|&k| self.due[k]);
        for &k in &done[..count] { itds.collect(k); self.armed &= !(1 << k); }
        // Every free one two frames ahead or more, for the next frame it runs in.
        let n = self.frames as u64;
        for k in 0..self.frames {
            let ahead = (k as u64 + n - self.now % n) % n;
            if self.armed & 1 << k != 0 || ahead < 2 { continue; }
            let due = self.now + ahead;
            self.late += (due - self.due[k]) / n - 1;
            self.due[k] = due;
            itds.arm(k);
            self.armed |= 1 << k;
        }
    }
}
