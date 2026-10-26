use nrl_lab::{
    close,
    experiment::{self, Dataset, Run},
    model::{Definition, Model, ObjectData, ObjectGradients},
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn definition() -> Definition {
    Definition::parse(include_bytes!(
        "../../../tests/fixtures/model-v0/definition.json"
    ))
    .unwrap()
}
fn plain() -> Model {
    Model::from_definition(
        Definition::parse(include_bytes!(
            "../../../tests/fixtures/model-v0/plain.json"
        ))
        .unwrap(),
    )
    .unwrap()
}
fn data() -> Dataset {
    Dataset::parse(include_bytes!(
        "../../../tests/fixtures/training-v0/data.json"
    ))
    .unwrap()
}
fn config() -> Run {
    Run::parse(include_bytes!(
        "../../../tests/fixtures/training-v0/run.json"
    ))
    .unwrap()
}
fn oracle() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/training-v0/expected.json"
    ))
    .unwrap()
}
fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "nrl-training-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn compare(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            close(&[a.as_f64().unwrap()], &[b.as_f64().unwrap()]).unwrap();
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                compare(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (key, a) in a {
                compare(a, &b[key]);
            }
        }
        _ => assert_eq!(a, b),
    }
}
fn tensors(data: &mut ObjectData) -> Vec<&mut [f64]> {
    match data {
        ObjectData::Affine(a) => vec![&mut a.weights, &mut a.bias],
        ObjectData::Gated(a) => {
            let mut result: Vec<&mut [f64]> = vec![&mut a.gate, &mut a.up, &mut a.down];
            if let Some(s) = &mut a.spline {
                result.push(&mut s.values);
            } else {
                result.push(&mut []);
            }
            if let Some(f) = &mut a.factors {
                result.extend(f.tensors_mut());
            }
            if let Some(b) = &mut a.bias {
                result.extend(b.tensors_mut());
            }
            result
        }
    }
}

#[test]
fn full_graph_gradients_and_single_tied_parameter_step_match_external_torch() {
    let expected = oracle();
    let f = &expected["full_graph"];
    let input = vector(&f["input"]);
    let dy = vector(&f["dy"]);
    let source = definition();
    let model = Model::from_definition(source.clone()).unwrap();
    let result = model.backward(&input, &dy, 65536).unwrap();
    close(&result.output, &vector(&f["output"])).unwrap();
    close(&result.input_gradient, &vector(&f["input_gradient"])).unwrap();
    compare(
        &serde_json::to_value(&result.gradients).unwrap(),
        &f["gradients"],
    );
    let mut next = source;
    next.objects = model
        .sgd_parameters(&result.gradients, f["learning_rate"].as_f64().unwrap())
        .unwrap();
    let updated = Model::from_definition(next).unwrap();
    close(
        &updated.forward(&input).unwrap(),
        &vector(&f["after_output"]),
    )
    .unwrap();
    assert_eq!(result.gradients.len(), 7);
    assert_eq!(model.manifest().descriptor.tied_objects[0].object, "embed");
    let base = model.object("embed").unwrap();
    let after = updated.object("embed").unwrap();
    if let (ObjectData::Affine(a), ObjectData::Affine(b), ObjectGradients::Affine { weights, .. }) =
        (base, after, &result.gradients["embed"])
    {
        close(
            &b.weights,
            &a.weights
                .iter()
                .zip(weights)
                .map(|(w, g)| w - 0.001 * g)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    } else {
        panic!("expected affine shared object");
    }
}

#[test]
fn every_complete_graph_parameter_and_input_has_an_independent_finite_difference() {
    let expected = oracle();
    let f = &expected["full_graph"];
    let x = vector(&f["input"]);
    let dy = vector(&f["dy"]);
    let source = definition();
    let model = Model::from_definition(source.clone()).unwrap();
    let backward = model.backward(&x, &dy, 65536).unwrap();
    let objective = |definition: Definition, input: &[f64]| {
        Model::from_definition(definition)
            .unwrap()
            .forward(input)
            .unwrap()
            .iter()
            .zip(&dy)
            .map(|(a, b)| a * b)
            .sum::<f64>()
    };
    let verify = |actual: f64, expected: f64| {
        assert!(
            (actual - expected).abs() <= 1e-6 + 1e-5 * expected.abs(),
            "finite difference {actual} != analytic {expected}"
        );
    };
    for i in 0..x.len() {
        let mut plus = x.clone();
        let mut minus = x.clone();
        plus[i] += 1e-6;
        minus[i] -= 1e-6;
        verify(
            (objective(source.clone(), &plus) - objective(source.clone(), &minus)) / 2e-6,
            backward.input_gradient[i],
        );
    }
    for (key, g) in &backward.gradients {
        for (tensor, gradient) in g.tensors().into_iter().enumerate() {
            for (index, expected) in gradient.iter().enumerate() {
                let mut plus = source.clone();
                let mut minus = source.clone();
                tensors(plus.objects.get_mut(key).unwrap())[tensor][index] += 1e-6;
                tensors(minus.objects.get_mut(key).unwrap())[tensor][index] -= 1e-6;
                verify(
                    (objective(plus, &x) - objective(minus, &x)) / 2e-6,
                    *expected,
                );
            }
        }
    }
}

#[test]
fn duplicate_add_inputs_accumulate_twice_and_relu_zero_has_declared_subgradient() {
    let mut source = Definition::parse(include_bytes!(
        "../../../tests/fixtures/model-v0/plain.json"
    ))
    .unwrap();
    source.descriptor.configuration.nodes[1].op = nrl_lab::model::Operation::Add {
        left: "hidden".into(),
        right: "hidden".into(),
    };
    let model = Model::from_definition(source.clone()).unwrap();
    let x = [0.2, -0.7];
    let dy = [0.4];
    let backward = model.backward(&x, &dy, 65536).unwrap();
    if let ObjectGradients::Affine { bias, .. } = &backward.gradients["hidden"] {
        close(bias, &[0.32, -0.16, 0.24, 0.08]).unwrap();
    } else {
        panic!("expected affine gradient");
    }
    source.descriptor.configuration.nodes[1].op = nrl_lab::model::Operation::Relu {
        input: "hidden".into(),
    };
    if let ObjectData::Affine(a) = source.objects.get_mut("hidden").unwrap() {
        a.weights.fill(0.0);
        a.bias.fill(0.0);
    }
    let model = Model::from_definition(source).unwrap();
    let backward = model.backward(&x, &dy, 65536).unwrap();
    assert_eq!(backward.input_gradient, vec![0.0; 2]);
    assert!(backward.gradients["hidden"]
        .tensors()
        .iter()
        .all(|v| v.iter().all(|x| *x == 0.0)));
}

#[test]
fn gradient_inventory_shapes_numerics_and_training_budgets_fail_without_mutation() {
    let model = plain();
    let x = [0.2, -0.7];
    let dy = [0.4];
    let before = model.model_id().to_owned();
    let output = model.forward(&x).unwrap();
    assert!(model.backward(&x, &dy, 1).unwrap_err().contains("budget"));
    assert!(model.backward(&x, &dy, (1 << 20) + 1).is_err());
    assert!(model.backward(&x, &[], 65536).is_err());
    assert!(model.backward(&x, &[f64::INFINITY], 65536).is_err());
    let g = model.backward(&x, &dy, 65536).unwrap().gradients;
    for rate in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(model.sgd_parameters(&g, rate).is_err());
    }
    let mut bad = g.clone();
    bad.remove("hidden");
    assert!(model.sgd_parameters(&bad, 0.1).is_err());
    let mut bad = g.clone();
    if let ObjectGradients::Affine { weights, .. } = bad.get_mut("hidden").unwrap() {
        weights.pop();
    }
    assert!(model.sgd_parameters(&bad, 0.1).is_err());
    let mut bad = g.clone();
    if let ObjectGradients::Affine { weights, .. } = bad.get_mut("hidden").unwrap() {
        weights[0] = f64::MAX;
    }
    assert!(model.sgd_parameters(&bad, 2.0).is_err());
    let mut bad = g.clone();
    if let ObjectGradients::Affine { weights, .. } = bad.get_mut("hidden").unwrap() {
        weights[0] = f64::NAN;
    }
    assert!(model.sgd_parameters(&bad, 0.1).is_err());
    assert_eq!(model.model_id(), before);
    assert_eq!(model.forward(&x).unwrap(), output);
}

#[test]
fn complete_regression_history_selected_model_and_held_out_outputs_match_torch() {
    let expected = oracle();
    let expected = &expected["regression"];
    let initial = plain();
    let experiment = experiment::execute(&initial, data(), config()).unwrap();
    assert_eq!(experiment.record.selected_epoch, expected["selected_epoch"]);
    for (row, expected) in experiment
        .record
        .history
        .iter()
        .zip(expected["history"].as_array().unwrap())
    {
        close(
            &[row.train.mse, row.validation.mse],
            &[
                expected["train_mse"].as_f64().unwrap(),
                expected["validation_mse"].as_f64().unwrap(),
            ],
        )
        .unwrap();
    }
    assert_eq!(experiment.record.history.len(), 129);
    let test: Vec<_> = experiment
        .dataset
        .samples
        .iter()
        .filter(|r| r.split == "test")
        .flat_map(|r| r.input.clone())
        .collect();
    close(
        &experiment.selected.forward(&test).unwrap(),
        &vector(&expected["selected_test_output"]),
    )
    .unwrap();
    for key in ["hidden", "readout"] {
        let a = experiment.selected.object(key).unwrap();
        let a = match a {
            ObjectData::Affine(a) => {
                json!({"kind":"affine","data":{"weights":a.weights,"bias":a.bias}})
            }
            _ => unreachable!(),
        };
        compare(&a, &expected["selected_parameters"][key]);
    }
    assert!(experiment.record.selected.test.mse < experiment.record.baseline.test.mse);
    assert!(experiment.record.selected.train.mse < experiment.record.baseline.train.mse);
    assert_eq!(experiment.record.graph_forward_rows, 9272);
    let repeated = experiment::execute(&initial, data(), config()).unwrap();
    assert_eq!(
        serde_json::to_vec(&experiment.record).unwrap(),
        serde_json::to_vec(&repeated.record).unwrap()
    );
    assert_eq!(experiment.selected.model_id(), repeated.selected.model_id());
}

fn toy() -> (Model, Dataset, Run) {
    let mut source = Definition::parse(include_bytes!(
        "../../../tests/fixtures/model-v0/plain.json"
    ))
    .unwrap();
    source.objects = BTreeMap::from([(
        "weight".into(),
        ObjectData::Affine(nrl_lab::model::AffineData {
            schema: "nrl.affine/0".into(),
            shape: [1, 1],
            weights: vec![0.0],
            bias: vec![],
        }),
    )]);
    source.descriptor.configuration.input_width = 1;
    source.descriptor.configuration.nodes = vec![nrl_lab::model::Node {
        id: "prediction".into(),
        op: nrl_lab::model::Operation::Affine {
            input: "input".into(),
            object: "weight".into(),
        },
    }];
    let mut dataset = data();
    dataset.input_width = 1;
    dataset.output_width = 1;
    dataset.samples = [
        ("train", 1.0, 1.0),
        ("validation", 2.0, 0.3),
        ("test", 3.0, 0.45),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (split, x, y))| experiment::Sample {
        id: format!("row-{i}"),
        group: format!("group-{i}"),
        split: split.into(),
        input: vec![x],
        target: vec![y],
    })
    .collect();
    let mut run = config();
    run.epochs = 6;
    run.learning_rate = 0.1;
    (Model::from_definition(source).unwrap(), dataset, run)
}

#[test]
fn validation_selects_an_earlier_checkpoint_and_test_targets_cannot_select_parameters() {
    let (model, dataset, run) = toy();
    let result = experiment::execute(&model, dataset.clone(), run.clone()).unwrap();
    assert_eq!(result.record.selected_epoch, 1);
    assert_eq!(result.record.history.len(), 7);
    let mut changed = dataset.clone();
    changed.samples[2].target[0] = 1000.0;
    let changed = experiment::execute(&model, changed, run.clone()).unwrap();
    assert_eq!(result.record.selected_epoch, changed.record.selected_epoch);
    assert_eq!(result.record.after_objects, changed.record.after_objects);
    assert_eq!(result.record.history, changed.record.history);
    assert_ne!(
        result.record.selected.test.mse,
        changed.record.selected.test.mse
    );
    let mut changed = dataset.clone();
    changed.samples[1].target[0] = 0.0;
    let changed = experiment::execute(&model, changed, run.clone()).unwrap();
    assert_eq!(changed.record.selected_epoch, 0);
    assert!(changed.record.selected.validation.near_zero_target_energy);
    assert!(changed.record.selected.validation.nmse.is_finite());
    let mut tied = dataset;
    for row in &mut tied.samples {
        row.target.fill(0.0);
    }
    let tied = experiment::execute(&model, tied, run).unwrap();
    assert_eq!(tied.record.selected_epoch, 0);
    let root = directory();
    let saved = root.join("tied");
    tied.save(&saved).unwrap();
    assert_eq!(experiment::check_saved(&saved).unwrap().selected_epoch, 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dataset_leakage_versions_shapes_sources_and_work_limits_are_refused_before_training() {
    let (model, valid, run) = toy();
    let check = |d: Dataset, r: Run, word: &str| {
        let error = experiment::execute(&model, d, r).err().unwrap();
        assert!(error.contains(word), "{error}");
    };
    let mut d = valid.clone();
    d.samples[1].group = d.samples[0].group.clone();
    check(d, run.clone(), "group");
    let mut d = valid.clone();
    d.samples[1].input = d.samples[0].input.clone();
    check(d, run.clone(), "duplicate input");
    let mut d = valid.clone();
    d.samples[0].input[0] = 0.0;
    d.samples[1].input[0] = -0.0;
    check(d, run.clone(), "duplicate input");
    let mut d = valid.clone();
    d.samples[1].id = d.samples[0].id.clone();
    check(d, run.clone(), "duplicate sample");
    let mut d = valid.clone();
    d.samples[1].input.push(0.0);
    check(d, run.clone(), "shape");
    let mut d = valid.clone();
    d.samples[1].target[0] = f64::NAN;
    check(d, run.clone(), "numerical");
    let mut d = valid.clone();
    d.source.licence.clear();
    check(d, run.clone(), "licence");
    let mut d = valid.clone();
    d.samples[2].split = "validation".into();
    check(d, run.clone(), "nonempty");
    let mut r = run.clone();
    r.max_forward_rows = 1;
    check(valid.clone(), r, "work budget");
    let mut r = run.clone();
    r.max_training_values = 1;
    check(valid.clone(), r, "training-value budget");
    let mut r = run.clone();
    r.epochs = 257;
    check(valid.clone(), r, "run budget");
    let mut r = run.clone();
    r.learning_rate = f64::INFINITY;
    check(valid.clone(), r, "run budget");
    let mut r = run;
    r.optimizer = "adam".into();
    check(valid, r, "optimizer");
    assert!(Dataset::parse(br#"{"schema":"nrl.regression-data/100"}"#)
        .unwrap_err()
        .contains("unsupported"));
    assert!(
        Dataset::parse(br#"{"schema":"nrl.regression-data/0","schema":"other"}"#)
            .unwrap_err()
            .contains("duplicate")
    );
    assert!(Run::parse(br#"{"schema":"nrl.regression-run/100"}"#)
        .unwrap_err()
        .contains("unsupported"));
    let mut unknown = serde_json::to_value(data()).unwrap();
    unknown["unexpected"] = json!(true);
    assert!(Dataset::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
    assert!(Dataset::parse(&vec![b' '; (16 << 20) + 1]).is_err());
}

#[test]
fn saved_experiment_cli_replays_identically_preserves_provenance_and_exports_independently() {
    let root = directory();
    let input = root.join("input");
    let output = root.join("experiment");
    let replay = root.join("replay");
    let mut initial = plain();
    let mut replacement = BTreeMap::new();
    let mut readout = initial.object("readout").unwrap().clone();
    if let ObjectData::Affine(a) = &mut readout {
        a.bias[0] += 0.01;
    }
    replacement.insert("readout".into(), readout);
    initial = initial
        .overlay(nrl_lab::model::Replacements {
            schema: "nrl.replacements/0".into(),
            source: config().output_source,
            reason: "Synthetic pre-existing history".into(),
            objects: replacement,
        })
        .unwrap()
        .standalone()
        .unwrap();
    initial.save(&input).unwrap();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/training-v0");
    let invoke = |args: &[&std::ffi::OsStr]| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_nrl-lab"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let reported = invoke(&[
        "train-regression".as_ref(),
        input.as_os_str(),
        fixture.join("data.json").as_os_str(),
        fixture.join("run.json").as_os_str(),
        output.as_os_str(),
    ]);
    let checked = experiment::check_saved(&output).unwrap();
    assert_eq!(reported, serde_json::to_value(checked).unwrap());
    let repeated = invoke(&[
        "replay-regression".as_ref(),
        output.as_os_str(),
        replay.as_os_str(),
    ]);
    assert_eq!(reported, repeated);
    assert_eq!(
        fs::read(output.join("training-record.json")).unwrap(),
        fs::read(replay.join("training-record.json")).unwrap()
    );
    assert_eq!(
        invoke(&["check-experiment".as_ref(), output.as_os_str()]),
        reported
    );
    let selected = nrl_lab::model::load(&output.join("selected-model"), None).unwrap();
    assert_eq!(selected.manifest().history, initial.manifest().history);
    assert_eq!(
        &selected.manifest().sources[..initial.manifest().sources.len()],
        initial.manifest().sources
    );
    assert!(selected
        .manifest()
        .sources
        .iter()
        .all(|s| !s.notices.is_empty()));
    let checkpoint = fs::read(output.join("training-record.json")).unwrap();
    let mut corrupted = checkpoint.clone();
    corrupted.push(b' ');
    fs::write(output.join("training-record.json"), corrupted).unwrap();
    assert!(experiment::check_saved(&output).is_err());
    fs::write(output.join("training-record.json"), checkpoint).unwrap();
    let failure = std::process::Command::new(env!("CARGO_BIN_EXE_nrl-lab"))
        .arg("replay-regression")
        .arg(&output)
        .arg(&replay)
        .output()
        .unwrap();
    assert!(!failure.status.success());
    let independent = root.join("standalone");
    fs::rename(output.join("selected-model"), &independent).unwrap();
    let prediction = selected.forward(&[0.2, -0.7]).unwrap();
    drop(selected);
    drop(initial);
    fs::remove_dir_all(&input).unwrap();
    fs::remove_dir_all(&output).unwrap();
    fs::remove_dir_all(&replay).unwrap();
    close(
        &nrl_lab::model::load(&independent, None)
            .unwrap()
            .forward(&[0.2, -0.7])
            .unwrap(),
        &prediction,
    )
    .unwrap();
    fs::remove_dir_all(root).unwrap();
}
