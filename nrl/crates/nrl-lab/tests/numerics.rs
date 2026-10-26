use nrl_lab::{check_case, close, evaluate, load, save, Artifact, MAX_ARTIFACT_BYTES};
use nrl_repr::Spline;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/f64-v1.json")).unwrap()
}

fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}

fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nrl-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    dir
}

#[test]
fn supported_blocks_match_torch_forward_backward_and_one_step() {
    for case in fixtures()["cases"].as_array().unwrap() {
        check_case(case).unwrap();
    }
}

#[test]
fn standalone_spline_matches_reference_at_knots_and_in_both_tails() {
    let f = &fixtures()["spline"];
    let low = vector(&f["low"]);
    let high = vector(&f["high"]);
    let values = vector(&f["values"]);
    let spline = Spline::new(2, 4, &low, &high, &values).unwrap();
    let mut actual = Vec::new();
    let mut dx = Vec::new();
    let mut dv = vec![0.0; values.len()];
    for (i, x) in vector(&f["input"]).iter().enumerate() {
        let sample = spline.sample(i % 2, *x).unwrap();
        actual.push(sample.value);
        dx.push(sample.derivative);
        dv[sample.left] += 1.0 - sample.fraction;
        dv[sample.left + 1] += sample.fraction;
    }
    close(&actual, &vector(&f["output"])).unwrap();
    close(&dx, &vector(&f["input_gradient"])).unwrap();
    close(&dv, &vector(&f["values_gradient"])).unwrap();
}

fn difference(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 2e-7 + 2e-5 * expected.abs(),
        "{actual} vs {expected}"
    );
}

#[test]
fn every_input_and_trainable_parameter_has_an_independent_numerical_gradient() {
    let f = fixtures();
    for case in f["cases"].as_array().unwrap() {
        let artifact: Artifact = serde_json::from_value(case["artifact"].clone()).unwrap();
        let input = vector(&case["input"]);
        let target = vector(&case["target"]);
        let analytical = evaluate(&artifact, &input, &target).unwrap();
        let epsilon = 1e-6;
        for i in 0..input.len() {
            let mut plus = input.clone();
            plus[i] += epsilon;
            let mut minus = input.clone();
            minus[i] -= epsilon;
            difference(
                (evaluate(&artifact, &plus, &target).unwrap().loss
                    - evaluate(&artifact, &minus, &target).unwrap().loss)
                    / (2.0 * epsilon),
                analytical.input_gradient[i],
            );
        }
        for parameter in 0..4 {
            let gradients = match parameter {
                0 => &analytical.gradients.gate,
                1 => &analytical.gradients.up,
                2 => &analytical.gradients.down,
                _ => &analytical.gradients.values,
            };
            for (i, gradient) in gradients.iter().enumerate() {
                let mut plus = artifact.clone();
                let mut minus = artifact.clone();
                for (block, delta) in [(&mut plus, epsilon), (&mut minus, -epsilon)] {
                    match parameter {
                        0 => block.gate[i] += delta,
                        1 => block.up[i] += delta,
                        2 => block.down[i] += delta,
                        _ => block.spline.as_mut().unwrap().values[i] += delta,
                    }
                }
                difference(
                    (evaluate(&plus, &input, &target).unwrap().loss
                        - evaluate(&minus, &input, &target).unwrap().loss)
                        / (2.0 * epsilon),
                    *gradient,
                );
            }
        }
    }
}

#[test]
fn artifact_round_trips_exactly_and_never_overwrites() {
    let dir = directory();
    for (i, case) in fixtures()["cases"].as_array().unwrap().iter().enumerate() {
        let artifact: Artifact = serde_json::from_value(case["artifact"].clone()).unwrap();
        let input = vector(&case["input"]);
        let path = dir.join(format!("{i}.json"));
        save(&path, &artifact).unwrap();
        let bytes = fs::read(&path).unwrap();
        let reloaded = load(&path).unwrap();
        assert_eq!(
            artifact.forward(&input).unwrap(),
            reloaded.forward(&input).unwrap()
        );
        assert!(save(&path, &artifact).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn loader_refuses_versions_unknown_fields_duplicates_and_oversized_files() {
    let dir = directory();
    let path = dir.join("bad.json");
    let mut value = fixtures()["cases"][0]["artifact"].clone();
    value["schema"] = Value::String("nrl.block/99".into());
    fs::write(&path, value.to_string()).unwrap();
    assert!(load(&path).unwrap_err().contains("nrl.block/99"));
    value["schema"] = Value::String("nrl.block/1".into());
    value["unknown"] = Value::Bool(true);
    fs::write(&path, value.to_string()).unwrap();
    assert!(load(&path).is_err());
    fs::write(
        &path,
        "{\"schema\":\"nrl.block/0\",\"schema\":\"nrl.block/0\"}",
    )
    .unwrap();
    assert!(load(&path).is_err());
    fs::write(&path, vec![b' '; MAX_ARTIFACT_BYTES + 1]).unwrap();
    assert!(load(&path).unwrap_err().contains("byte limit"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_shapes_grids_and_finite_value_errors_are_controlled() {
    let artifact: Artifact =
        serde_json::from_value(fixtures()["cases"][1]["artifact"].clone()).unwrap();
    let mut bad = artifact.clone();
    bad.shape[0] = 0;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.shape[0] = usize::MAX;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.gate.pop();
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.spline.as_mut().unwrap().knots = 1;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.spline.as_mut().unwrap().high[0] = -1.0;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.spline.as_mut().unwrap().values[0] = f64::NAN;
    assert!(bad.validate().is_err());
    assert!(artifact.forward(&[]).is_err());
    assert!(artifact.forward(&[1.0]).is_err());
    assert!(artifact.forward(&[f64::INFINITY; 3]).is_err());
    assert!(evaluate(&artifact, &[1.0; 3], &[0.0; 2]).is_err());
    let result = evaluate(&artifact, &[0.1; 3], &[0.2; 3]).unwrap();
    assert!(artifact.step(&result.gradients, f64::INFINITY).is_err());
}
