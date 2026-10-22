//! The agent's telemetry (750-EFF-0014, effector/src/telemetry.rs): a schema 1 sample as the Effector server takes it,
//! rates only between two samples of the same processes, missing values left out, never sent as zero.
#![allow(dead_code)]
extern crate alloc;
#[path = "../effector/src/json.rs"] mod json;
#[path = "../effector/src/telemetry.rs"] mod telemetry;
#[path = "../libmind/src/json.rs"] mod reader;
use telemetry::*;

fn counters(uptime_ms: u64, busy_ns: u64, idle_ns: u64, effector_run_ns: u64, effector_started_ns: u64) -> Counters {
    Counters {
        cpus: 4, busy_ns, idle_ns, uptime_ms, memory_total: 8 << 30, memory_free: 6 << 30,
        services: vec![
            Load { name: "effector".into(), state: "running", pid: 38, run_ns: effector_run_ns, started_ns: effector_started_ns, memory: 3 << 20 },
            Load { name: "updater".into(), state: "stopped", ..Load::default() },
        ],
        volumes: vec![Space { name: "".into(), bytes: 1 << 30, free: 1 << 29 }, Space { name: "log".into(), bytes: 64 << 20, free: 60 << 20 }],
    }
}

const ID: Identity<'static> = Identity { sample_id: "s1792566386-65200", sampled_at: "2026-10-22T18:06:26Z", cpu_model: "Intel(R) Core(TM) i7-3615QM", arch: "x64", executed: 3, executed_total: 17 };
const CARDS: [Card; 2] = [Card { card: 0, wireless: false, up: false, mac: 0, address: 0 }, Card { card: 2, wireless: true, up: true, mac: 0x0000_a4b1_c2d3_e4f5, address: 0xC0A8_0D2A }];

fn text(now: &Counters, before: Option<&Counters>) -> String {
    let mut buf = vec![0u8; BODY_MAX];
    let body = String::from_utf8(sample(&mut buf, now, before, &CARDS, &ID).unwrap().to_vec()).unwrap();
    reader::walk(body.as_bytes(), &mut |_, _| Ok(())).expect("valid JSON");
    body
}

#[test]
fn the_first_sample_has_no_rates() {
    let body = text(&counters(65_200, 10_000_000_000, 30_000_000_000, 500_000_000, 3_000_000_000), None);
    assert!(body.starts_with(r#"{"schema":1,"sample_id":"s1792566386-65200","sampled_at":"2026-10-22T18:06:26Z","interval_seconds":30.000,"system":{"cpu_model":"Intel(R) Core(TM) i7-3615QM","architecture":"x64","logical_cpus":4,"uptime_seconds":65,"executed_requests":3,"executed_requests_total":17,"memory_total":8589934592,"memory_available":6442450944,"memory_used":2147483648},"#), "{body}");
    assert!(!body.contains("cpu_percent"), "{body}");
    assert!(body.contains(r#""disks":[{"name":"boot","kind":"volume","mount":"/","total_bytes":1073741824,"free_bytes":536870912},{"name":"log","kind":"volume","mount":"log:","total_bytes":67108864,"free_bytes":62914560}]"#), "{body}");
    assert!(body.contains(r#""interfaces":[{"name":"card0","type":"ethernet","state":"down"},{"name":"card2","type":"wireless","state":"up","mac_address":"a4:b1:c2:d3:e4:f5","ip_addresses":["192.168.13.42"]}]"#), "{body}");
    assert!(body.ends_with(r#""components":[{"id":"effector","name":"effector","program":"effector","state":"running","pids":[38],"working_set_bytes":3145728},{"id":"updater","name":"updater","program":"updater","state":"stopped","pids":[]}]}"#), "{body}");
}

#[test]
fn rates_come_from_two_samples() {
    let before = counters(65_200, 10_000_000_000, 30_000_000_000, 500_000_000, 3_000_000_000);
    // 30 s later: 4 CPUs busy 30 of 120 s (25%); the agent used 1.2 s of CPU time (1% of 4 CPUs x 30 s).
    let now = counters(95_200, 40_000_000_000, 120_000_000_000, 1_700_000_000, 3_000_000_000);
    let body = text(&now, Some(&before));
    assert!(body.contains(r#""interval_seconds":30.000,"#) && body.contains(r#""cpu_percent":25.000,"uptime_seconds":95,"#), "{body}");
    assert!(body.contains(r#""pids":[38],"cpu_percent":1.000,"working_set_bytes""#), "{body}");
    // The service restarted (another start time): no rate for it, the machine's still there.
    let restarted = counters(95_200, 40_000_000_000, 120_000_000_000, 1_700_000_000, 90_000_000_000);
    let body = text(&restarted, Some(&before));
    assert!(body.contains(r#""cpu_percent":25.000,"#) && body.contains(r#""pids":[38],"working_set_bytes""#), "{body}");
    // Another boot (uptime went back) or a gap over an hour: no rates at all.
    for later in [counters(5_000, 1, 1, 1, 1), counters(65_200 + 3_600_001, 40_000_000_000, 120_000_000_000, 1_700_000_000, 3_000_000_000)] {
        let body = text(&later, Some(&before));
        assert!(!body.contains("cpu_percent") && body.contains(r#""interval_seconds":30.000,"#), "{body}");
    }
}

#[test]
fn every_service_fits_and_values_stay_in_bounds() {
    let mut c = counters(1000, 0, 0, 0, 0);
    c.services = (0..48).map(|i| Load { name: format!("service{i:02}xxxxx"), state: "running", pid: 1000 + i, run_ns: u64::MAX, started_ns: 1, memory: u64::MAX }).collect();
    c.memory_free = c.memory_total + 1;
    let mut before = c.clone();
    before.uptime_ms = 999;
    for s in before.services.iter_mut() { s.run_ns = 0; }
    let body = text(&c, Some(&before));
    assert!(body.len() < BODY_MAX && !body.contains("cpu_percent\":100.001"), "{}", body.len());
    assert!(body.contains(r#""cpu_percent":100.000"#) && body.contains(r#""memory_available":8589934592,"memory_used":0"#), "{body}");
}
