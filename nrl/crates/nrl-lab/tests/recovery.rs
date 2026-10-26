use nrl_lab::{
    close,
    decoder::Decoder,
    model::Source,
    research::{
        plan::{self, Study},
        recovery::{self, Record},
        Corpus,
    },
};
use serde_json::{json, Value};
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

fn study() -> Study {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/study-v0/profile.json"
    ))
    .unwrap()
}

#[test]
fn actual_student_inputs_have_teacher_on_same_input_targets_and_simultaneous_updates() {
    let m = model();
    let c = corpus();
    let s = study();
    let a = plan::apply(&m, &c, &s, "mixed-linear-pruned").unwrap();
    let pairs =
        recovery::collect(&m, &a.model, &c, 64, "frozen-teacher-on-student-input/0").unwrap();
    let control =
        recovery::collect(&m, &a.model, &c, 64, "frozen-teacher-on-original-input/0").unwrap();
    pairs.verify(&m, &a.model, &c).unwrap();
    assert_eq!(pairs.layers[&0][0].input, control.layers[&0][0].input);
    assert_ne!(pairs.layers[&2][0].input, control.layers[&2][0].input);
    for (layer, docs) in &pairs.layers {
        for d in docs {
            close(
                &d.target,
                &m.dense(*layer).unwrap().forward(&d.input).unwrap(),
            )
            .unwrap();
        }
    }
    assert_ne!(pairs.layers[&2][0].target, control.layers[&2][0].target);
    let before = m.model_id().to_owned();
    let (updated, fits) = recovery::update(&a.model, &pairs, 0.001).unwrap();
    assert_eq!(fits.len(), 2);
    assert_eq!(before, m.model_id());
    for (layer, docs) in &pairs.layers {
        let x: Vec<_> = docs.iter().flat_map(|d| d.input.clone()).collect();
        let y: Vec<_> = docs.iter().flat_map(|d| d.target.clone()).collect();
        let old = a.model.patch(*layer).unwrap();
        let expected = old
            .step(&nrl_lab::evaluate(old, &x, &y).unwrap().gradients, 0.001)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&expected).unwrap(),
            serde_json::to_value(updated.patch(*layer).unwrap()).unwrap()
        );
    }
    let mut bad = pairs.clone();
    bad.layers.get_mut(&2).unwrap()[0].target[0] += 1.0;
    assert!(bad.verify(&m, &a.model, &c).is_err());
    assert!(recovery::update(&updated, &pairs, 0.001).is_err());
    let mut changed = c.clone();
    for d in &mut changed.documents {
        if d.split == "test" {
            d.text.push_str(" The end.");
        }
    }
    let other = recovery::collect(
        &m,
        &a.model,
        &changed,
        64,
        "frozen-teacher-on-student-input/0",
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&pairs.layers).unwrap(),
        serde_json::to_value(&other.layers).unwrap()
    );
    assert!(recovery::collect(
        &m,
        &a.model,
        &c,
        usize::MAX,
        "frozen-teacher-on-student-input/0"
    )
    .is_err());
    assert!(recovery::collect(&m, &a.model, &c, 64, "unknown").is_err());
}
#[test]
fn complete_recovery_selects_validation_rounds_and_replays_saved_pairs_models_and_metrics() {
    let m = model();
    let c = corpus();
    let s = study();
    let a = plan::apply(&m, &c, &s, "mixed-linear-pruned").unwrap();
    let run = recovery::run(&m, &a.model, c, s).unwrap();
    for arm in &run.record.arms {
        assert_eq!(arm.history.len(), 4);
        let selected = arm
            .history
            .iter()
            .min_by(|a, b| {
                a.validation_nll
                    .total_cmp(&b.validation_nll)
                    .then(a.round.cmp(&b.round))
            })
            .unwrap();
        assert_eq!(selected.round, arm.selected_round);
        assert_eq!(selected.student_model_id, arm.selected_raw_model_id);
    }
    assert_eq!(run.recommendation["test_used_for_selection"], false);
    assert_eq!(run.test.len(), 4);
    assert_eq!(run.pairs.len(), 6);
    assert_eq!(run.student.patches().len(), 2);
    let path = std::env::temp_dir().join(format!("nrl-recovery-{}", std::process::id()));
    run.save(&path).unwrap();
    assert!(run.save(&path).is_err());
    let repeat = recovery::replay(&m, &path).unwrap();
    assert_eq!(
        repeat.record.record_id().unwrap(),
        run.record.record_id().unwrap()
    );
    assert_eq!(repeat.student.model_id(), run.student.model_id());
    let mut damaged: Value =
        serde_json::from_slice(&std::fs::read(path.join("record.json")).unwrap()).unwrap();
    damaged["payload"]["arms"][0]["history"][1]["validation_nll"] = json!(0.0);
    std::fs::write(
        path.join("record.json"),
        serde_json::to_vec(&damaged).unwrap(),
    )
    .unwrap();
    assert!(Record::load(&path.join("record.json")).is_err());
    run.record.save(&path.join("record-copy.json")).unwrap();
    std::fs::remove_file(path.join("record.json")).unwrap();
    run.record.save(&path.join("record.json")).unwrap();
    let mut tied = run.record.clone();
    for arm in &mut tied.arms {
        let nll = arm.history[0].validation_nll;
        for r in &mut arm.history {
            r.validation_nll = nll;
        }
        arm.selected_round = 0;
        arm.selected_raw_model_id = arm.history[0].student_model_id.clone();
    }
    tied.validate().unwrap();
    tied.arms[0].selected_round = 1;
    assert!(tied.validate().is_err());
    let mut complete: Value =
        serde_json::from_slice(&std::fs::read(path.join("complete.json")).unwrap()).unwrap();
    complete["record_id"] = json!("a".repeat(64));
    std::fs::write(
        path.join("complete.json"),
        serde_json::to_vec(&complete).unwrap(),
    )
    .unwrap();
    assert!(recovery::replay(&m, &path).is_err());
    std::fs::remove_dir_all(path).unwrap();
}
