//! The server's `performance_measure` (750-EFF-0016): the Effector agents' fixed benchmarks, so a MIND machine's
//! numbers compare with the others' (Effector's agent/linux/performance_benchmark.go): `effector_bogomips_v1` on one
//! thread, `effector_memcpy_v1` and `effector_seqio_v1` writes and reads. The session runs it a slice at a time between
//! its heartbeats, so the agent stays online for its minute; each phase's rate counts the time its slices ran.
//! What a task's 16 MiB of memory (`HEAP_MAX_BYTES`) does not hold is changed or left out, and said so:
//! - the copy moves 4 MiB between two buffers, not 8 MiB (still beyond this kind of processor's last cache);
//! - the pointer chase (`effector_pointer_chase_v1`, which the server takes only over exactly 16 MiB) is left out;
//! - so is the parallel CPU phase, which needs threads the session has not.
//! No system calls: the clock and the disk come from the caller; host-tested in tests/effector_bench_host.rs.
use crate::json::Writer;
use alloc::vec;
use alloc::vec::Vec;

pub const CPU: &str = "effector_bogomips_v1";
pub const MEMORY: &str = "effector_memcpy_v1";
pub const DISK: &str = "effector_seqio_v1";
const BATCH: u64 = 4096;
const SEED: u64 = 0x6a09_e667_f3bc_c909;
const COPY_BYTES: usize = 4 << 20; // 8 MiB in the Linux agent: two such buffers and the rest would pass the task's 16 MiB
const DISK_PIECE: usize = 64 << 10; // 1 MiB a write on Linux; a slow disk would hold a 1 MiB write past a heartbeat
const DISK_WRAP: u64 = 16 << 20; // the file starts again at 16 MiB, as the Linux agent's

/// One batch of the CPU benchmark (`performanceBatch`).
pub fn batch(mut state: u64) -> u64 {
    for _ in 0..BATCH {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        state ^= state >> 33;
        state = state.wrapping_mul(0xff51afd7ed558ccd);
        state ^= state >> 33;
    }
    state
}

/// The machine's processor time when sampled (the gateway's counters): busy and idle over every CPU, and the agent's
/// own, in ns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cpu { pub busy_ns: u64, pub idle_ns: u64, pub own_ns: u64 }

/// What else ran around the benchmark (the server's `load`, as the Linux agent's performance_load.go reckons it): the
/// machine's busy share in a baseline second before, its share over the run, and that share less the agent's own.
#[derive(Clone, Copy, Debug, Default)]
pub struct Conditions { start: Option<Cpu>, first: Option<Cpu>, last: Option<Cpu>, before: Option<u64>, pub samples: u32 }

impl Conditions {
    /// The baseline second begins.
    pub fn begin(&mut self, c: Cpu) { self.start = Some(c); }
    /// The baseline second ends and the run begins.
    pub fn baseline(&mut self, c: Cpu) {
        self.before = self.start.and_then(|s| share(s, c)).map(|(cpu, _)| cpu);
        self.first = Some(c);
        self.last = Some(c);
    }
    /// A sample during the run, about each second.
    pub fn sample(&mut self, c: Cpu) {
        if self.last.and_then(|l| share(l, c)).is_some() { self.samples += 1; }
        self.last = Some(c);
    }
    /// The run's busy share and the share less the agent's own, in thousandths of a percent.
    fn over_run(&self) -> Option<(u64, u64)> { share(self.first?, self.last?) }
}

/// The busy share between two samples, and the busy share less the agent's own, in thousandths of a percent.
fn share(a: Cpu, b: Cpu) -> Option<(u64, u64)> {
    let busy = b.busy_ns.checked_sub(a.busy_ns)? as u128;
    let total = busy + b.idle_ns.checked_sub(a.idle_ns)? as u128;
    let own = (b.own_ns.saturating_sub(a.own_ns) as u128).min(busy);
    (total > 0).then(|| ((busy * 100_000 / total) as u64, ((busy - own) * 100_000 / total) as u64))
}

/// The machine's file for the disk phase.
pub trait Disk {
    /// Writes `data` at `offset` (0 starts the file again); false if it failed.
    fn write_at(&mut self, offset: u64, data: &[u8]) -> bool;
    /// Makes what was written durable.
    fn flush(&mut self) -> bool;
    /// Reads at `offset`; 0 at the end.
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> usize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase { Cpu, Memory, Write, Read, Done }

/// The benchmark, run a slice at a time. Times are the caller's milliseconds.
pub struct Bench {
    phase: Phase,
    /// Each phase's length in ms: CPU duration/3, memory /12, disk /12 each, as the Linux agent's.
    lengths: [u64; 4],
    ran_ms: u64,
    started_ms: Option<u64>,
    finished_ms: u64,
    state: u64,
    pub iterations: u64,
    pub cpu_ms: u64,
    copy: Option<(Vec<u8>, Vec<u8>)>,
    pub copied: u64,
    pub copy_ms: u64,
    piece: Vec<u8>,
    pub written: u64,
    pub write_ms: u64,
    pub read: u64,
    pub read_ms: u64,
    disk_failed: bool,
}

impl Bench {
    /// For the server's `duration_seconds` (1–300).
    pub fn new(duration_s: u64) -> Self {
        let d = duration_s.clamp(1, 300) * 1000;
        Self {
            phase: Phase::Cpu, lengths: [d / 3, d / 12, d / 12, d / 12], ran_ms: 0, started_ms: None, finished_ms: 0,
            state: SEED, iterations: 0, cpu_ms: 0, copy: None, copied: 0, copy_ms: 0, piece: Vec::new(), written: 0, write_ms: 0, read: 0, read_ms: 0, disk_failed: false,
        }
    }

    pub fn done(&self) -> bool { self.phase == Phase::Done }

    fn next(&mut self) {
        self.ran_ms = 0;
        self.phase = match self.phase { Phase::Cpu => Phase::Memory, Phase::Memory => Phase::Write, Phase::Write => Phase::Read, _ => Phase::Done };
        if self.phase == Phase::Read && self.disk_failed { self.phase = Phase::Done; }
    }

    /// Runs until `now()` passes `slice_ms` from the call; true once the benchmark is done.
    pub fn step(&mut self, now: &mut dyn FnMut() -> u64, slice_ms: u64, disk: &mut dyn Disk) -> bool {
        let start = now();
        self.started_ms.get_or_insert(start);
        let until = start + slice_ms;
        while self.phase != Phase::Done {
            let length = self.lengths[self.phase as usize];
            let left = length.saturating_sub(self.ran_ms);
            if left == 0 { self.leave(disk); self.next(); continue; }
            self.prepare();
            let end = until.min(now() + left);
            let began = now();
            self.work(now, end, disk);
            let spent = now().saturating_sub(began).max(1);
            self.ran_ms += spent;
            match self.phase { Phase::Cpu => self.cpu_ms += spent, Phase::Memory => self.copy_ms += spent, Phase::Write => self.write_ms += spent, Phase::Read => self.read_ms += spent, Phase::Done => {} }
            if now() >= until { break; }
        }
        if self.phase == Phase::Done && self.finished_ms == 0 { self.finished_ms = now().max(1); }
        self.done()
    }

    // A phase's buffers, made before its time is counted, as the Linux agent's are.
    fn prepare(&mut self) {
        match self.phase {
            Phase::Cpu if self.iterations == 0 => { let _ = core::hint::black_box(batch(SEED)); } // warms the loop
            Phase::Memory if self.copy.is_none() => self.copy = Some(((0..COPY_BYTES).map(|i| (i * 31 + 17) as u8).collect(), vec![0u8; COPY_BYTES])),
            Phase::Write | Phase::Read if self.piece.is_empty() => self.piece = (0..DISK_PIECE).map(|i| (i * 13 + 29) as u8).collect(),
            _ => {}
        }
    }

    // The phase's work until `end`, in units small enough to see the clock often.
    fn work(&mut self, now: &mut dyn FnMut() -> u64, end: u64, disk: &mut dyn Disk) {
        match self.phase {
            Phase::Cpu => while now() < end { self.state = batch(self.state); self.iterations += BATCH; },
            Phase::Memory => {
                let Some((mut a, mut b)) = self.copy.take() else { return };
                while now() < end { b.copy_from_slice(&a); self.copied += COPY_BYTES as u64; core::mem::swap(&mut a, &mut b); }
                self.copy = Some((a, b));
            }
            Phase::Write => {
                while now() < end && !self.disk_failed {
                    if !disk.write_at(self.written % DISK_WRAP, &self.piece) { self.disk_failed = true; }
                    self.written += DISK_PIECE as u64;
                }
            }
            Phase::Read => {
                let limit = self.written.min(DISK_WRAP);
                while now() < end && limit > 0 {
                    let n = disk.read_at(self.read % limit, &mut self.piece);
                    if n == 0 { self.disk_failed = true; break; }
                    self.read += n as u64;
                }
            }
            Phase::Done => {}
        }
    }

    // What a phase leaves behind: its memory freed, the written file made durable.
    fn leave(&mut self, disk: &mut dyn Disk) {
        match self.phase {
            Phase::Memory => self.copy = None,
            Phase::Write => if self.disk_failed || self.written == 0 || !disk.flush() { self.disk_failed = true; },
            Phase::Read => self.piece = Vec::new(),
            _ => {}
        }
    }

    /// The result as the server takes it (Effector's AcceptPerformance): rates in thousandths, durations in seconds.
    pub fn result<'b>(&self, buf: &'b mut [u8], logical: u16, cpu_model: &str, arch: &str, version: &str, conditions: Option<&Conditions>) -> Result<&'b [u8], crate::json::Error> {
        // Thousandths per second; never 0, which the server refuses for a measured rate.
        let per_s = |amount: u128, ms: u64| ((amount * 1000 * 1000 / ms.max(1) as u128) as u64).max(1);
        let mut w = Writer::new(buf);
        w.object()?;
        w.field_str("status", "completed")?;
        w.field_u64("exit_code", 0)?;
        w.field_str("stdout", "")?;
        w.field_str("stderr", "")?;
        w.field_bool("truncated", false)?;
        w.field_str("run_as", "effector")?;
        w.field_str("benchmark", CPU)?;
        w.field_decimal("bogomips", (per_s(self.iterations as u128, self.cpu_ms) / 1_000_000).max(1))?;
        w.field_decimal("duration_seconds", self.cpu_ms)?;
        w.field_u64("iterations", self.iterations)?;
        let mut hex = [0u8; 16];
        for (i, d) in hex.iter_mut().enumerate() { *d = b"0123456789abcdef"[(self.state >> (60 - 4 * i) & 15) as usize]; }
        w.field_str("checksum", core::str::from_utf8(&hex).unwrap_or(""))?;
        w.field_u64("worker_threads", 1)?;
        w.field_u64("logical_processors", logical.max(1) as u64)?;
        w.field_str("cpu_model", cpu_model)?;
        w.field_str("architecture", arch)?;
        w.field_str("agent_version", version)?;
        w.field_decimal("total_duration_seconds", self.finished_ms.saturating_sub(self.started_ms.unwrap_or(0)))?;
        if self.copied > 0 {
            w.field_str("memory_benchmark", MEMORY)?;
            w.field_decimal("memory_mib_per_second", (per_s(self.copied as u128, self.copy_ms) >> 20).max(1))?;
        }
        if let Some(c) = conditions {
            // One slot, the measurement's own: no other queue ran beside it.
            w.key("load")?;
            w.object()?;
            w.field_u64("samples", c.samples as u64)?;
            w.field_u64("occupied_queues_start", 1)?;
            w.field_u64("occupied_queues_max", 1)?;
            w.field_u64("other_queues_max", 0)?;
            if let Some(before) = c.before { w.field_decimal("cpu_before_percent", before)?; }
            let run = c.over_run();
            if let Some((cpu, background)) = run {
                w.field_decimal("cpu_average_percent", cpu)?;
                w.field_decimal("background_cpu_average_percent", background)?;
            }
            w.field_bool("under_load", c.before.is_some_and(|b| b >= 10_000) || run.is_some_and(|(_, bg)| bg >= 10_000))?;
            w.end()?;
        }
        if !self.disk_failed && self.written > 0 && self.read > 0 {
            w.field_str("disk_benchmark", DISK)?;
            w.field_decimal("disk_read_mib_per_second", (per_s(self.read as u128, self.read_ms) >> 20).max(1))?;
            w.field_decimal("disk_write_mib_per_second", (per_s(self.written as u128, self.write_ms) >> 20).max(1))?;
        }
        w.end()?;
        w.finish()
    }
}
