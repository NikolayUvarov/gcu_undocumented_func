//! The agent's performance_measure (750-EFF-0016, effector/src/bench.rs): the Effector agents' algorithms, run a slice at
//! a time on a fake clock, and a result the server's AcceptPerformance takes.
#![allow(dead_code)]
extern crate alloc;
#[path = "../effector/src/json.rs"] mod json;
#[path = "../effector/src/bench.rs"] mod bench;
#[path = "../libmind/src/json.rs"] mod reader;
use bench::*;

#[derive(Default)]
struct File { bytes: Vec<u8>, flushed: bool, fail: bool }
impl Disk for File {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> bool {
        if self.fail { return false; }
        let end = offset as usize + data.len();
        if self.bytes.len() < end { self.bytes.resize(end, 0); }
        self.bytes[offset as usize..end].copy_from_slice(data);
        true
    }
    fn flush(&mut self) -> bool { self.flushed = true; true }
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> usize {
        let start = (offset as usize).min(self.bytes.len());
        let n = (self.bytes.len() - start).min(out.len());
        out[..n].copy_from_slice(&self.bytes[start..start + n]);
        n
    }
}

#[test]
fn the_cpu_loop_is_the_linux_agents() {
    // performanceBatch from the seed, in Go: state = state*6364136223846793005 + 1442695040888963407; xorshift 33; ...
    let mut state = 0x6a09_e667_f3bc_c909u64;
    for _ in 0..4096 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        state ^= state >> 33; state = state.wrapping_mul(0xff51afd7ed558ccd); state ^= state >> 33;
    }
    assert_eq!(batch(0x6a09_e667_f3bc_c909), state);
}

#[test]
fn a_run_in_slices_gives_a_result_the_server_takes() {
    // A clock that moves 1 ms each time it is read: every phase advances and ends.
    let mut t = 0u64;
    let mut now = || { t += 1; t };
    let mut b = Bench::new(3); // 1000 ms of CPU, 250 of copying, 250 each of writing and reading
    let mut disk = File::default();
    let mut slices = 0;
    while !b.step(&mut now, 50, &mut disk) { slices += 1; assert!(slices < 10_000); }
    assert!(slices > 20, "run in slices, the heartbeat between them");
    assert!(b.iterations > 0 && b.copied > 0 && b.written > 0 && b.read > 0 && disk.flushed);
    assert!(b.cpu_ms >= 1000 && b.cpu_ms < 1100, "{}", b.cpu_ms);
    let mut buf = vec![0u8; 4096];
    let mut c = Conditions::default();
    c.begin(Cpu { busy_ns: 0, idle_ns: 0, own_ns: 0 });
    c.baseline(Cpu { busy_ns: 400_000_000, idle_ns: 7_600_000_000, own_ns: 0 }); // 5% busy in the baseline second
    c.sample(Cpu { busy_ns: 2_400_000_000, idle_ns: 13_600_000_000, own_ns: 1_000_000_000 });
    c.sample(Cpu { busy_ns: 4_400_000_000, idle_ns: 19_600_000_000, own_ns: 2_000_000_000 }); // the run: 25% busy, 12.5% the agent's
    let body = String::from_utf8(b.result(&mut buf, 8, "Intel(R) Core(TM) i7-3615QM", "x64", "0.1.0", Some(&c)).unwrap().to_vec()).unwrap();
    assert!(body.contains(r#""load":{"samples":2,"occupied_queues_start":1,"occupied_queues_max":1,"other_queues_max":0,"cpu_before_percent":5.000,"cpu_average_percent":25.000,"background_cpu_average_percent":12.500,"under_load":true}"#), "{body}");
    reader::walk(body.as_bytes(), &mut |_, _| Ok(())).expect("valid JSON");
    for field in [r#""status":"completed""#, r#""benchmark":"effector_bogomips_v1""#, r#""worker_threads":1"#, r#""logical_processors":8"#,
                  r#""memory_benchmark":"effector_memcpy_v1""#, r#""disk_benchmark":"effector_seqio_v1""#] {
        assert!(body.contains(field), "{field} in {body}");
    }
    let checksum = body.split(r#""checksum":""#).nth(1).unwrap();
    assert!(checksum[..16].bytes().all(|c| c.is_ascii_hexdigit()) && checksum.as_bytes()[16] == b'"', "16 hex digits: {body}");
    assert!(!body.contains("cpu_parallel") && !body.contains("memory_latency"), "left out: no threads, no 16 MiB to spare");
}

#[test]
fn a_disk_that_fails_leaves_the_disk_numbers_out() {
    let mut t = 0u64;
    let mut now = || { t += 1; t };
    let mut b = Bench::new(1);
    let mut disk = File { fail: true, ..File::default() };
    while !b.step(&mut now, 100, &mut disk) {}
    let mut buf = vec![0u8; 4096];
    let body = String::from_utf8(b.result(&mut buf, 1, "", "arm64", "0.1.0", None).unwrap().to_vec()).unwrap();
    assert!(!body.contains("disk_benchmark") && body.contains("memory_benchmark"), "{body}");
}
