//! 351-ASR-0006: seeded fuzzing of the boot-record reader and the slot choice (bootloader/src/slots.rs; MC-9.1, 9.3,
//! 12.2). Records come from the disk, which anyone who can write it may change: the reader must never panic and accept
//! only its one encoding; the choice must never panic, must boot a real slot, and a record it writes must count as newer
//! than the one it follows, or the trial's count-down stops. A run is evidence of its inputs, not a proof.
//! MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the fixed seed and count.
#[path = "../bootloader/src/slots.rs"]
mod slots;
use slots::{plan, spent, Record, RECORD};
use std::panic::{self, AssertUnwindSafe};

const SEED: u64 = 0x351A_0006;
const ITERATIONS: usize = 200_000;

fn seed() -> u64 { std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(SEED) }
fn iterations() -> usize { std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(ITERATIONS) }

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    fn below(&mut self, n: u64) -> u64 { if n == 0 { 0 } else { self.next() % n } }
    // A sequence number, often at the edges.
    fn sequence(&mut self) -> u64 { [0, 1, u64::MAX, u64::MAX - 1, 1 << 63][self.below(5) as usize].wrapping_add(if self.below(2) == 0 { 0 } else { self.next() }) }
    fn slot(&mut self) -> u8 { [b'A', b'B'][self.below(2) as usize] }
}

fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|e| e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())
}

// A record the reader would accept: any sequence, a real slot, a fallback of none or a slot, any tries.
fn record(rng: &mut Rng) -> Record {
    Record { sequence: rng.sequence(), slot: rng.slot(), fallback: [0, b'A', b'B'][rng.below(3) as usize], tries: rng.below(256) as u8, confirmed: rng.below(2) == 0 }
}

#[test]
fn the_reader_accepts_only_its_one_encoding() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(seed() | 1);
    let mut findings = Vec::new();
    let (mut accepted, mut runs) = (0, 0);
    for _ in 0..iterations() {
        // A valid record changed in a few bytes, or random bytes of any length near a sector's.
        let mut data = if rng.below(4) != 0 { record(&mut rng).encode().to_vec() } else { (0..rng.below(RECORD as u64 + 8)).map(|_| rng.next() as u8).collect() };
        if !data.is_empty() { for _ in 0..rng.below(4) { let i = rng.below(data.len() as u64) as usize; data[i] ^= 1 << rng.below(8); } }
        runs += 1;
        match guarded(|| Record::parse(&data)) {
            Err(p) => findings.push(format!("parse panicked ({p}) on {} bytes", data.len())),
            Ok(Some(r)) => { accepted += 1; if r.encode().as_slice() != data.as_slice() { findings.push(format!("accepted {r:?} from bytes it does not encode to")); } }
            Ok(None) => {}
        }
        if findings.len() > 10 { break; }
    }
    panic::set_hook(hook);
    println!("boot records: {runs} inputs, {accepted} accepted (seed {:#x})", seed());
    assert!(findings.is_empty(), "boot-record findings:\n{}", findings.join("\n"));
}

#[test]
fn the_choice_always_boots_a_slot_and_counts_down() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(seed().rotate_left(17) | 1);
    let mut findings = std::collections::BTreeMap::<String, String>::new();
    for _ in 0..iterations() {
        let records = [if rng.below(5) == 0 { None } else { Some(record(&mut rng)) }, if rng.below(5) == 0 { None } else { Some(record(&mut rng)) }];
        let p = match guarded(|| plan(records)) { Ok(p) => p, Err(e) => { findings.entry(format!("plan panicked: {e}")).or_insert(format!("{records:?}")); continue; } };
        if !matches!(p.order[0], b'A' | b'B') || !matches!(p.order[1], 0 | b'A' | b'B') || p.order[0] == p.order[1] {
            findings.entry("the order is not a slot, then none or the other slot".into()).or_insert(format!("{records:?} -> {p:?}"));
        }
        if p.trial && p.write.is_none() { findings.entry("a trial without a record written first".into()).or_insert(format!("{records:?} -> {p:?}")); }
        if let (Some(chosen), Some((file, written))) = (p.chosen, p.write) {
            let followed = records[chosen].unwrap();
            if file != 1 - chosen { findings.entry("the record is written over the one followed".into()).or_insert(format!("{records:?} -> {p:?}")); }
            if written.sequence <= followed.sequence { findings.entry("the written record is not newer than the one followed: the count-down stops".into()).or_insert(format!("{records:?} -> {p:?}")); }
            match guarded(|| spent((file, written))) {
                Ok((_, after)) if after.sequence <= written.sequence => { findings.entry("the spent record is not newer than the trial's".into()).or_insert(format!("{written:?} -> {after:?}")); }
                Ok(_) => {}
                Err(e) => { findings.entry(format!("spent panicked: {e}")).or_insert(format!("{written:?}")); }
            }
        }
    }
    panic::set_hook(hook);
    println!("slot choices: {} pairs of records (seed {:#x}), {} kinds of finding", iterations(), seed(), findings.len());
    for (kind, example) in &findings { println!("  {kind}\n    {example}"); }
    assert!(findings.is_empty(), "slot-choice findings: {:?}", findings.keys().collect::<Vec<_>>());
}
