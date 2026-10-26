use nrl_lab::{check_case, close, load, read_json, save, Artifact};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 4 && args[1] == "pack-model" {
        let definition = nrl_lab::model::Definition::load(Path::new(&args[2]))?;
        let model = nrl_lab::model::Model::from_definition(definition)?;
        model.save(Path::new(&args[3]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&model.info()?).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.len() == 5 && args[1] == "export-overlay" {
        let base = nrl_lab::model::load(Path::new(&args[2]), None)?;
        let replacements = nrl_lab::model::Replacements::load(Path::new(&args[3]))?;
        let overlay = base.overlay(replacements)?;
        overlay.save(Path::new(&args[4]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&overlay.info()?).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if ((args.len() == 3 || args.len() == 4) && args[1] == "check-model")
        || ((args.len() == 4 || args.len() == 5) && args[1] == "export-standalone")
        || ((args.len() == 5 || args.len() == 6) && args[1] == "run-model")
    {
        let base_index = match args[1].as_str() {
            "check-model" => 3,
            "export-standalone" => 4,
            _ => 5,
        };
        let base = if args.len() > base_index {
            Some(nrl_lab::model::load(Path::new(&args[base_index]), None)?)
        } else {
            None
        };
        let model = nrl_lab::model::load(Path::new(&args[2]), base.as_ref())?;
        if args[1] == "run-model" {
            let input: Vec<f64> = serde_json::from_value(read_json(Path::new(&args[3]))?)
                .map_err(|e| e.to_string())?;
            model.run_to_file(&input, Path::new(&args[4]))?;
        } else if args[1] == "export-standalone" {
            model.standalone()?.save(Path::new(&args[3]))?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&model.info()?).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.len() == 5 && args[1] == "import-qsl" {
        let imported = nrl_lab::qsl::import_directory(Path::new(&args[2]), &args[4])?;
        imported.save(Path::new(&args[3]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(imported.manifest()).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.len() == 3 && args[1] == "check-block-set" {
        let set = nrl_lab::qsl::load(Path::new(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(set.manifest()).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.len() != 4 || args[1] != "demo" {
        return Err("usage: nrl-lab demo FIXTURE_JSON NEW_OUTPUT_DIRECTORY | import-qsl OVERLAY_DIRECTORY NEW_OUTPUT_DIRECTORY WEIGHT_LICENCE | check-block-set DIRECTORY | pack-model DEFINITION NEW_DIRECTORY | export-overlay BASE REPLACEMENTS NEW_DIRECTORY | check-model MODEL [BASE] | run-model MODEL INPUT NEW_OUTPUT [BASE] | export-standalone MODEL NEW_DIRECTORY [BASE]".into());
    }
    let fixture = read_json(Path::new(&args[2]))?;
    if fixture["schema"] != "nrl.fixture/5" {
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
    let report = serde_json::json!({"schema": "nrl.demo-report/1", "version": env!("CARGO_PKG_VERSION"),
        "profile": "host-cpu-scalar-f64", "architecture": std::env::consts::ARCH, "os": std::env::consts::OS,
        "archive_sha256": fixture["archive_sha256"], "seed": fixture["seed"], "checks": checks,
        "fixture_round_trips": cases.len(), "elapsed_seconds": started.elapsed().as_secs_f64(),
        "measurement": "One fixture check and one round trip per case; elapsed time is not a model benchmark",
        "native_execution": false, "unimplemented": ["model adapters", "nnd integration"]});
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
