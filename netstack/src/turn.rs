// One fair driver-wait budget per client-service turn (550-NET-0032).
pub const TURN_MS: u32 = 250;
pub const CARD_MS: u32 = 2000;
pub const WINDOW_MS: u64 = 5000;
const BUCKET_MS: u64 = 500;
const BUCKETS: usize = (WINDOW_MS / BUCKET_MS) as usize + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit { pub index: usize, pub started_ms: u64, pub remaining_ms: u32 }

#[derive(Clone, Copy)]
struct Bucket { tag: u64, spent_ms: u32 }

#[derive(Clone, Copy)]
struct Account { buckets: [Bucket; BUCKETS] }
impl Account {
    const fn new() -> Self { Self { buckets: [Bucket { tag: 0, spent_ms: 0 }; BUCKETS] } }

    // Keep the entire completion bucket until even its latest instant is at least five seconds old.
    fn remaining(&mut self, now_ms: u64) -> u32 {
        let tag = now_ms / BUCKET_MS;
        let mut spent = 0u64;
        for bucket in &mut self.buckets {
            if tag.saturating_sub(bucket.tag) >= BUCKETS as u64 { bucket.spent_ms = 0; }
            spent += bucket.spent_ms as u64;
        }
        (CARD_MS as u64).saturating_sub(spent) as u32
    }

    fn charge(&mut self, started_ms: u64, now_ms: u64) {
        let tag = now_ms / BUCKET_MS;
        let bucket = &mut self.buckets[tag as usize % BUCKETS];
        if bucket.tag != tag { *bucket = Bucket { tag, spent_ms: 0 }; }
        bucket.spent_ms = bucket.spent_ms.saturating_add(now_ms.saturating_sub(started_ms).min(u32::MAX as u64) as u32);
    }
}

pub struct Turn<const CARDS: usize> {
    next: usize, left: usize, end_ms: u64, active: Option<Visit>, accounts: [Account; CARDS],
    last_ms: u64, clock_failed: bool,
}
impl<const CARDS: usize> Turn<CARDS> {
    pub const fn new() -> Self {
        Self { next: 0, left: 0, end_ms: 0, active: None, accounts: [Account::new(); CARDS], last_ms: 0, clock_failed: false }
    }
    pub fn begin(&mut self, now_ms: u64) {
        if !self.advance(now_ms) { return; }
        self.finish(now_ms);
        self.left = CARDS; self.end_ms = now_ms.saturating_add(TURN_MS as u64);
    }

    // Charge the elapsed visit, including CPU work; never interpret quota exhaustion as device death.
    pub fn finish(&mut self, now_ms: u64) {
        if !self.advance(now_ms) { return; }
        if let Some(visit) = self.active.take() {
            self.accounts[visit.index].charge(visit.started_ms, now_ms);
        }
    }

    fn advance(&mut self, now_ms: u64) -> bool {
        if now_ms < self.last_ms { self.clock_failed = true; }
        self.last_ms = now_ms;
        !self.clock_failed
    }

    // Advance only for a visited slot; a turn that ends early resumes at its next slot.
    pub fn next(&mut self, now_ms: u64) -> Option<Visit> {
        if !self.advance(now_ms) || self.active.is_some() { return None; }
        let turn_left = self.end_ms.saturating_sub(now_ms).min(TURN_MS as u64);
        if turn_left == 0 { return None; }
        while self.left != 0 {
            let index = self.next;
            self.next = (self.next + 1) % CARDS;
            self.left -= 1;
            let remaining_ms = (self.accounts[index].remaining(now_ms) as u64).min(turn_left) as u32;
            if remaining_ms == 0 { continue; }
            let visit = Visit { index, started_ms: now_ms, remaining_ms };
            self.active = Some(visit);
            return Some(visit);
        }
        None
    }
}
