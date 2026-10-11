//! Host tests of usb_host/src/iso_ring.rs (158): the order in which an isochronous ring's iTDs are collected and armed,
//! against a model of the controller. QEMU has no high-speed isochronous device, so this is the ring's only test before
//! a real camera.
#[path = "../usb_host/src/iso_ring.rs"]
mod iso_ring;
use iso_ring::{Descriptors, Ring};

// The controller: in frame `now` it runs iTD now % frames if that one is armed, and stamps it with the frame.
struct Model { frames: usize, now: u64, active: Vec<bool>, stamp: Vec<Option<u64>>, collected: Vec<u64>, too_near: usize }

impl Model {
    fn new(frames: usize, now: u64) -> Self { Self { frames, now, active: vec![false; frames], stamp: vec![None; frames], collected: Vec::new(), too_near: 0 } }
    fn run_frame(&mut self) {
        let k = self.now as usize % self.frames;
        if self.active[k] { self.active[k] = false; self.stamp[k] = Some(self.now); }
        self.now += 1;
    }
    // The frame list's index, as FRINDEX gives it (1024 entries).
    fn frame(&self) -> usize { (self.now % 1024) as usize }
}

impl Descriptors for Model {
    fn active(&self, k: usize) -> bool { self.active[k] }
    fn collect(&mut self, k: usize) { if let Some(s) = self.stamp[k].take() { self.collected.push(s); } }
    fn arm(&mut self, k: usize) {
        // The controller may hold the iTD of this frame and the next: arming one of those races it.
        if (k + self.frames - self.now as usize % self.frames) % self.frames < 2 { self.too_near += 1; }
        self.active[k] = true;
    }
}

// A fixed sequence of gaps between the driver's looks, from `low` to `high` frames.
fn gaps(seed: u64, low: u64, high: u64) -> impl FnMut() -> u64 {
    let mut state = seed;
    move || { state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); low + (state >> 33) % (high - low + 1) }
}

// The driver looks, the controller runs frames for a gap, and so on; also the least the ring must deliver: after a look
// at frame t every frame from t + 2 to t + frames - 1 has its iTD armed.
fn run(frames: usize, start: u64, total: u64, mut gap: impl FnMut() -> u64) -> (Model, Ring, u64) {
    let mut model = Model::new(frames, start);
    let mut ring = Ring::new(frames, model.frame());
    let mut least = 0;
    while model.now < start + total {
        ring.pump(model.frame(), &mut model);
        let g = gap();
        least += g.min(frames as u64).saturating_sub(2);
        for _ in 0..g { model.run_frame(); }
    }
    ring.pump(model.frame(), &mut model);
    (model, ring, least)
}

#[test]
fn looked_at_often_enough_every_frame_comes_once_and_in_order() {
    for (frames, high) in [(32usize, 10u64), (64, 30), (8, 6), (4, 2)] {
        for start in [0u64, 5, 1021, 3000] {
            let (model, ring, _) = run(frames, start, 5000, gaps(start + frames as u64, 1, high));
            let got = &model.collected;
            assert!(got.len() > 4000, "{frames} frames from {start}: {} collected", got.len());
            assert_eq!(got[0], start + 2, "the first armed frame is two past the start");
            assert!(got.windows(2).all(|w| w[1] == w[0] + 1), "{frames} frames from {start}: a frame lost or out of order");
            assert_eq!((model.too_near, ring.late), (0, 0), "{frames} frames from {start}");
        }
    }
}

#[test]
fn looked_at_too_late_frames_are_lost_but_the_order_holds() {
    for frames in [4usize, 32, 64] {
        let (model, ring, least) = run(frames, 7, 20_000, gaps(frames as u64, 1, 3 * frames as u64));
        let got = &model.collected;
        assert!(got.windows(2).all(|w| w[1] > w[0]), "{frames} frames: out of order or twice");
        assert_eq!(model.too_near, 0, "{frames} frames: an iTD armed where the controller may hold it");
        assert!(got.len() as u64 >= least, "{frames} frames: {} delivered, at least {least} could be", got.len());
        // Frames the controller ran unarmed since the first delivered one. The last look has not yet counted those of the
        // two iTDs it could not arm again (a few at most).
        let lost = model.now - got[0] - got.len() as u64;
        assert!(ring.late > 0 && ring.late <= lost && lost <= ring.late + 8, "{frames} frames: {} counted late, {lost} lost", ring.late);
    }
}

#[test]
fn a_look_in_the_same_frame_changes_nothing() {
    let mut model = Model::new(32, 100);
    let mut ring = Ring::new(32, model.frame());
    ring.pump(model.frame(), &mut model);
    let armed = ring.armed;
    assert_eq!(armed.count_ones(), 30, "every iTD but this frame's and the next");
    ring.pump(model.frame(), &mut model);
    assert_eq!((ring.armed, model.collected.len()), (armed, 0));
    for _ in 0..3 { model.run_frame(); }
    ring.pump(model.frame(), &mut model);
    assert_eq!(model.collected, vec![102], "frame 102 ran; 100 and 101 were never armed");
}
