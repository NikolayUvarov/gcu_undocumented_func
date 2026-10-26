use nrl_lab::{
    close,
    decoder::{self, Decoder},
    model::Source,
    research::{local, Calibration, Corpus, LocalConfig, LocalState, Transform},
};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
fn model() -> Decoder {
    let s: Value = serde_json::from_slice(include_bytes!(
        "../../../reference/tinyllamas-260k/decoder-source.json"
    ))
    .unwrap();
    let sources: Vec<Source> = serde_json::from_value(s["sources"].clone()).unwrap();
    Decoder::from_bytes(
        include_bytes!("../../../reference/tinyllamas-260k/stories260K.bin").to_vec(),
        include_bytes!("../../../reference/tinyllamas-260k/tok512.bin").to_vec(),
        sources,
    )
    .unwrap()
}
fn corpus() -> Corpus {
    Corpus::parse(include_bytes!(
        "../../../tests/fixtures/local-v0/corpus.json"
    ))
    .unwrap()
}
fn config() -> LocalConfig {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/local-v0/config.json"
    ))
    .unwrap()
}
fn oracle() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/local-v0/expected.json"
    ))
    .unwrap()
}
fn reference_corpus() -> Corpus {
    let f: Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/decoder-v0.json")).unwrap();
    let mut c = corpus();
    c.documents.truncate(3);
    for (i, split) in ["train", "validation", "test"].into_iter().enumerate() {
        c.documents[i].split = split.into();
        c.documents[i].text = f["cases"][i]["text"].as_str().unwrap().into();
    }
    c
}
fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "nrl-local-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn vector(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
#[test]
fn actual_teacher_calibration_has_split_provenance_and_no_test_activations() {
    let m = model();
    let c = corpus();
    let cal = Calibration::collect(&m, c.clone(), 0, 128).unwrap();
    cal.verify_teacher(&m).unwrap();
    assert!(cal.batch("test").is_err());
    assert!(cal.train.iter().all(|p| c
        .documents
        .iter()
        .any(|d| d.id == p.document_id && d.split == "train")));
    let mut changed = c.clone();
    for d in &mut changed.documents {
        if d.split == "test" {
            d.text.push_str(" Good night.");
        }
    }
    let other = Calibration::collect(&m, changed, 0, 128).unwrap();
    assert_eq!(
        cal.train
            .iter()
            .flat_map(|p| p.input.clone())
            .collect::<Vec<_>>(),
        other
            .train
            .iter()
            .flat_map(|p| p.input.clone())
            .collect::<Vec<_>>()
    );
    let mut bad = c.clone();
    bad.documents[4].group = bad.documents[0].group.clone();
    assert!(Calibration::collect(&m, bad, 0, 128)
        .err()
        .unwrap()
        .contains("group"));
    let mut bad = c.clone();
    bad.documents[4].text = bad.documents[0].text.clone();
    assert!(Calibration::collect(&m, bad, 0, 128)
        .err()
        .unwrap()
        .contains("duplicate"));
    let mut bad = cal.clone();
    bad.train[0].input[0] += 1.0;
    assert!(bad.verify_teacher(&m).is_err());
    let mut bad = cal.clone();
    bad.train[0].tokens = vec![1; 95];
    bad.train[0].input = vec![0.0; 95 * 64];
    bad.train[0].target = vec![0.0; 95 * 64];
    assert!(bad.verify_teacher(&m).is_err());
    assert!(Calibration::collect(&m, c.clone(), 99, 128).is_err());
    assert!(Calibration::collect(&m, c, 0, 257).is_err());
}
#[test]
fn real_model_channel_selection_and_four_step_sgd_match_independent_torch() {
    let m = model();
    let cal = Calibration::collect(&m, reference_corpus(), 0, 32).unwrap();
    let mut cfg = config();
    cfg.transform.representation = "pruned".into();
    cfg.transform.width = 8;
    cfg.transform.knots = 0;
    cfg.max_steps = 4;
    let mut state = LocalState::start(&m, cal, cfg).unwrap();
    state.advance(4).unwrap();
    let expected = oracle();
    assert_eq!(
        serde_json::to_value(&state.retained_indices).unwrap(),
        expected["retained_indices"]
    );
    assert_eq!(state.best_step, expected["selected_step"]);
    for (actual, expected) in state
        .history
        .iter()
        .zip(expected["history"].as_array().unwrap())
    {
        close(
            &[actual.train.mse, actual.validation.mse],
            &[
                expected["train_mse"].as_f64().unwrap(),
                expected["validation_mse"].as_f64().unwrap(),
            ],
        )
        .unwrap();
    }
    close(&state.best.gate, &vector(&expected["selected"]["gate"])).unwrap();
    close(&state.best.up, &vector(&expected["selected"]["up"])).unwrap();
    close(&state.best.down, &vector(&expected["selected"]["down"])).unwrap();
}
#[test]
fn all_compressed_initializers_share_training_channel_map_and_keep_explicit_bounds() {
    let m = model();
    let cal = Calibration::collect(&m, corpus(), 0, 32).unwrap();
    let mut mapping = None;
    for representation in ["pruned", "spline-linear", "spline-cubic", "svd"] {
        let spec = Transform {
            schema: "nrl.transform/0".into(),
            representation: representation.into(),
            width: 8,
            knots: if representation.starts_with("spline") {
                8
            } else {
                0
            },
            rank: if representation == "svd" { 2 } else { 0 },
        };
        let (a, indices) = local::initialize(&m, &cal, &spec).unwrap();
        a.validate().unwrap();
        if let Some(previous) = &mapping {
            assert_eq!(previous, &indices);
        } else {
            mapping = Some(indices);
        }
        let mut cfg = config();
        cfg.transform = spec;
        cfg.max_steps = 2;
        let mut state = LocalState::start(&m, cal.clone(), cfg).unwrap();
        state.advance(2).unwrap();
        assert_eq!(state.step, 2);
        if representation == "svd" {
            assert!(state.initial.selection.is_none());
            assert_eq!(state.retained_indices.len(), 8);
        } else {
            assert_eq!(state.initial.selection.as_ref().unwrap().indices.len(), 8);
        }
        if representation.starts_with("spline") {
            assert!((0.0..=1.0).contains(
                &state.history[state.best_step]
                    .validation
                    .outside_grid_fraction
                    .unwrap()
            ));
        }
    }
    let mut cfg = config();
    cfg.transform.rank = 3;
    assert!(cfg.validate().is_err());
    let mut cfg = config();
    cfg.learning_rate = f64::NAN;
    assert!(cfg.validate().is_err());
}
#[test]
fn saved_optimizer_data_order_current_and_best_state_resume_exactly() {
    let m = model();
    let cal = Calibration::collect(&m, corpus(), 0, 48).unwrap();
    let mut cfg = config();
    cfg.max_steps = 4;
    cfg.transform.width = 8;
    let mut uninterrupted = LocalState::start(&m, cal.clone(), cfg.clone()).unwrap();
    uninterrupted.advance(4).unwrap();
    let root = directory();
    let checkpoint = root.join("checkpoint.json");
    let mut partial = LocalState::start(&m, cal, cfg).unwrap();
    partial.advance(2).unwrap();
    partial.save(&checkpoint).unwrap();
    assert!(partial.save(&checkpoint).is_err());
    let mut resumed = LocalState::load(&checkpoint).unwrap();
    resumed.advance(4).unwrap();
    assert_eq!(
        resumed.checkpoint_id().unwrap(),
        uninterrupted.checkpoint_id().unwrap()
    );
    assert_eq!(resumed.history, uninterrupted.history);
    let selected = resumed.export(&m).unwrap();
    let path = root.join("selected");
    selected.save(&path).unwrap();
    let loaded = decoder::load(&path).unwrap();
    assert_eq!(loaded.model_id(), selected.model_id());
    assert_ne!(loaded.model_id(), m.model_id());
    let previous = fs::read(&checkpoint).unwrap();
    let mut bad: Value = serde_json::from_slice(&previous).unwrap();
    bad["payload"]["step"] = json!(3);
    fs::write(&checkpoint, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(LocalState::load(&checkpoint)
        .err()
        .unwrap()
        .contains("hash"));
    assert!(resumed.advance(3).is_err());
    assert!(resumed.advance(65).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn calibration_start_resume_export_are_independent_cli_operations() {
    let root = directory();
    let base = root.join("base");
    model().save(&base).unwrap();
    let cal = root.join("cal.json");
    let partial = root.join("partial.json");
    let complete = root.join("complete.json");
    let selected = root.join("selected");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/local-v0");
    let invoke = |args: &[&std::ffi::OsStr]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_nrl-lab"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let collected = invoke(&[
        "calibrate-decoder".as_ref(),
        base.as_os_str(),
        fixtures.join("corpus.json").as_os_str(),
        "0".as_ref(),
        "48".as_ref(),
        cal.as_os_str(),
    ]);
    assert_eq!(collected["test_activations"], 0);
    invoke(&[
        "train-local".as_ref(),
        base.as_os_str(),
        cal.as_os_str(),
        fixtures.join("config.json").as_os_str(),
        partial.as_os_str(),
        "2".as_ref(),
    ]);
    let report = invoke(&[
        "resume-local".as_ref(),
        partial.as_os_str(),
        complete.as_os_str(),
        "8".as_ref(),
    ]);
    assert_eq!(report["completed_steps"], 8);
    invoke(&[
        "export-local".as_ref(),
        base.as_os_str(),
        complete.as_os_str(),
        selected.as_os_str(),
    ]);
    assert!(decoder::load(&selected).unwrap().patch(0).is_some());
    fs::remove_dir_all(root).unwrap();
}
