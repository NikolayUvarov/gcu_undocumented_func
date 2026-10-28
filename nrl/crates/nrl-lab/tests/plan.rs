use nrl_lab::{
    decoder::Decoder,
    model::Source,
    research::{
        evaluate::{self, Report},
        plan::{self, Candidate, Study},
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
fn manual_mixed_plan_composes_real_original_teacher_blocks() {
    let m = model();
    let c = corpus();
    let s = study();
    let a = plan::apply(&m, &c, &s, "mixed-linear-pruned").unwrap();
    assert_eq!(a.model.patches().len(), 2);
    assert_eq!(a.model.patch(0).unwrap().representation, "spline-linear");
    assert_eq!(a.model.patch(2).unwrap().representation, "pruned");
    assert_eq!(
        a.model.base_content_id().unwrap(),
        m.base_content_id().unwrap()
    );
    for i in 0..m.config().layers {
        assert_eq!(m.dense(i).unwrap().gate, a.model.dense(i).unwrap().gate);
    }
    let tokens = c.tokenize(&m).unwrap();
    let student = a.model.forward(&tokens[4][..5]).unwrap();
    assert_ne!(student, m.forward(&tokens[4][..5]).unwrap());
    assert!(student.iter().all(|v| v.is_finite()));
    for state in &a.states {
        state.calibration.verify_teacher(&m).unwrap();
        assert_eq!(state.config.max_steps, 8);
        assert!(state.calibration.batch("test").is_err());
    }
    let mut changed = c.clone();
    for d in &mut changed.documents {
        if d.split == "test" {
            d.text.push_str(" Good night.");
        }
    }
    let mut matching = s.clone();
    matching.corpus_id = changed.data_id().unwrap();
    let other = plan::apply(&m, &changed, &matching, "mixed-linear-pruned").unwrap();
    for (x, y) in a.states.iter().zip(&other.states) {
        assert_eq!(
            serde_json::to_value(&x.best).unwrap(),
            serde_json::to_value(&y.best).unwrap()
        );
        assert_eq!(x.best_step, y.best_step);
    }
    let tmp = std::env::temp_dir().join(format!("nrl-plan-{}", std::process::id()));
    a.save(&tmp).unwrap();
    assert!(a.save(&tmp).is_err());
    let loaded = nrl_lab::decoder::load(&tmp.join("model")).unwrap();
    assert_eq!(loaded.model_id(), a.model.model_id());
    std::fs::remove_dir_all(tmp).unwrap();
}
#[test]
fn plans_refuse_duplicate_out_of_budget_and_other_source_inputs() {
    let m = model();
    let c = corpus();
    let s = study();
    for kind in [
        "duplicate",
        "entries",
        "steps",
        "nan",
        "count",
        "layer",
        "width",
        "protocol",
    ] {
        let mut bad = s.clone();
        match kind {
            "duplicate" => bad.plans.push(bad.plans[0].clone()),
            "entries" => {
                let entry = bad.plans[0].entries[0].clone();
                bad.plans[0].entries.push(entry);
            }
            "steps" => bad.local_steps = usize::MAX,
            "nan" => bad.local_learning_rate = f64::NAN,
            "count" => bad.plans.clear(),
            "layer" => bad.plans[0].entries[0].layer = usize::MAX,
            "width" => bad.plans[0].entries[0].transform.width = usize::MAX,
            _ => bad.recovery.protocol = "other".into(),
        };
        assert!(bad.verify(&m, &c).is_err(), "{kind}");
    }
    let mut other = c.clone();
    other.documents[0].text.push_str(" One day.");
    assert!(s.verify(&m, &other).is_err());
    assert!(plan::apply(&m, &c, &s, "unknown").is_err());
}
fn times(r: &mut Report, t: f64) {
    let a = r.timing.as_mut().unwrap();
    a.seconds = vec![t; 3];
    a.minimum_seconds = t;
    a.median_seconds = t;
    a.p95_seconds = t;
}
#[test]
fn conservative_validation_selection_and_pareto_keep_negative_cases() {
    let m = model();
    let c = corpus();
    let s = study();
    let mut b =
        evaluate::evaluate_language(&m, &m, &c, "validation", Some(s.measurement.clone())).unwrap();
    times(&mut b, 1.0);
    let mut cs: Vec<_> = s
        .plans
        .iter()
        .map(|p| Candidate {
            plan_id: p.id.clone(),
            validation: None,
            refusal: Some("explicit synthetic numerical refusal".into()),
            training: None,
        })
        .collect();
    assert_eq!(plan::select(&b, &cs, &s).unwrap()["selected_plan"], "dense");
    let mut r = b.clone();
    r.model_id = "a".repeat(64);
    times(&mut r, 0.5);
    r.quality.value += 0.1;
    r.model_information["package_bytes"] = json!(1100000);
    cs[0].validation = Some(r);
    cs[0].refusal = None;
    let selected = plan::select(&b, &cs, &s).unwrap();
    assert_eq!(selected["selected_plan"], s.plans[0].id);
    assert_eq!(selected["test_used_for_selection"], false);
    assert!(selected["pareto"]
        .as_array()
        .unwrap()
        .contains(&json!(s.plans[0].id)));
    cs[0].validation.as_mut().unwrap().quality.value += 1.0;
    assert_eq!(plan::select(&b, &cs, &s).unwrap()["selected_plan"], "dense");
    let mut test = b.clone();
    test.profile.split = "test".into();
    assert!(plan::select(&test, &cs, &s).is_err());
    cs.pop();
    assert!(plan::select(&b, &cs, &s).is_err());
}
