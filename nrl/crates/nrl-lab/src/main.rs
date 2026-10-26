use nrl_lab::{check_case, close, load, read_json, save, Artifact};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || args[1] != "demo" {
        return Err("usage: nrl-lab demo FIXTURE_JSON NEW_OUTPUT_DIRECTORY".into());
    }
    let fixture = read_json(Path::new(&args[2]))?;
    if fixture["schema"] != "nrl.fixture/3" {
        return Err("unsupported fixture schema".into());
    }
    let cases = fixture["cases"].as_array().ok_or("missing fixture cases")?;
    let started = Instant::now();
    let checks: Vec<_> = cases.iter().map(check_case).collect::<Result<_, _>>()?;
    let dir = Path::new(&args[3]);
    fs::create_dir(dir).map_err(|e| e.to_string())?;
    for (index, case) in cases.iter().enumerate() {
        let artifact: Artifact =
            serde_json::from_value(case["artifact"].clone()).map_err(|e| e.to_string())?;
        let input: Vec<f64> =
            serde_json::from_value(case["input"].clone()).map_err(|e| e.to_string())?;
        let file = dir.join(format!("block-{index}.json"));
        save(&file, &artifact)?;
        let loaded = load(&file)?;
        if close(&artifact.forward(&input)?, &loaded.forward(&input)?)? != 0.0 {
            return Err("artifact round trip changed outputs".into());
        }
    }
    let report = serde_json::json!({"schema": "nrl.demo-report/0", "version": env!("CARGO_PKG_VERSION"),
        "profile": "host-cpu-scalar-f64", "architecture": std::env::consts::ARCH, "os": std::env::consts::OS,
        "archive_sha256": fixture["archive_sha256"], "seed": fixture["seed"], "checks": checks,
        "fixture_round_trips": cases.len(), "elapsed_seconds": started.elapsed().as_secs_f64(),
        "measurement": "One fixture check and one round trip per case; elapsed time is not a model benchmark",
        "native_execution": false, "unimplemented": ["biased/affine block conversion", "model adapters", "nnd integration"]});
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dir.join("report.json"))
        .map_err(|e| e.to_string())?
        .write_all(
            serde_json::to_string_pretty(&report)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("nrl-lab: {error}");
        std::process::exit(1);
    }
}
