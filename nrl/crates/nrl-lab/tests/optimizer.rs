use nrl_lab::{
    close,
    optimizer::{Config, State, Tensors},
};
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/adaptation-v0/optimizer-oracle.json"
    ))
    .unwrap()
}
#[test]
fn adamw_parameters_and_moments_match_pytorch_and_exact_continuation() {
    let f = fixture();
    let c: Config = serde_json::from_value(f["config"].clone()).unwrap();
    let mut p: Tensors = serde_json::from_value(f["initial"].clone()).unwrap();
    let mut state = State::start(c, &p).unwrap();
    for (i, row) in f["steps"].as_array().unwrap().iter().enumerate() {
        let g = serde_json::from_value(row["gradients"].clone()).unwrap();
        state.update(&mut p, &g).unwrap();
        for (actual, name) in [
            (&p, "parameters"),
            (&state.first, "first"),
            (&state.second, "second"),
        ] {
            let expected: Tensors = serde_json::from_value(row[name].clone()).unwrap();
            for (k, v) in actual {
                close(v, &expected[k]).unwrap();
            }
        }
        assert_eq!(state.step, i + 1);
        let encoded = serde_json::to_vec(&state).unwrap();
        let restored: State = serde_json::from_slice(&encoded).unwrap();
        restored.validate(&p).unwrap();
        assert_eq!(encoded, serde_json::to_vec(&restored).unwrap());
        state = restored;
    }
    assert!(state
        .update(
            &mut p,
            &serde_json::from_value(f["steps"][0]["gradients"].clone()).unwrap()
        )
        .is_err());
}
#[test]
fn invalid_gradient_wrong_parameter_and_nonfinite_updates_are_atomic() {
    let f = fixture();
    let c = serde_json::from_value(f["config"].clone()).unwrap();
    let p: Tensors = serde_json::from_value(f["initial"].clone()).unwrap();
    let state = State::start(c, &p).unwrap();
    let gradients: Tensors = serde_json::from_value(f["steps"][0]["gradients"].clone()).unwrap();
    for mode in 0..5 {
        let mut s = state.clone();
        let mut params = p.clone();
        let mut g = gradients.clone();
        match mode {
            0 => g.get_mut("body.0").unwrap()[0] = f64::NAN,
            1 => {
                g.remove("head.0");
            }
            2 => params.get_mut("body.0").unwrap()[0] += 0.1,
            3 => g.get_mut("body.0").unwrap()[0] = f64::MAX,
            _ => s.second.get_mut("body.0").unwrap()[0] = -0.1,
        }
        let before = serde_json::to_vec(&s).unwrap();
        let beforep = serde_json::to_vec(&params).unwrap();
        assert!(s.update(&mut params, &g).is_err());
        assert_eq!(before, serde_json::to_vec(&s).unwrap());
        assert_eq!(beforep, serde_json::to_vec(&params).unwrap());
    }
}
#[test]
fn recovery_checkpoint_resume_is_identical_and_test_changes_do_not_select() {
    use nrl_lab::{
        decoder::Decoder,
        model::Source,
        research::{adaptation, plan, Corpus},
    };
    let v: Value = serde_json::from_slice(include_bytes!(
        "../../../reference/tinyllamas-260k/decoder-source.json"
    ))
    .unwrap();
    let sources: Vec<Source> = serde_json::from_value(v["sources"].clone()).unwrap();
    let m = Decoder::from_bytes(
        include_bytes!("../../../reference/tinyllamas-260k/stories260K.bin").to_vec(),
        include_bytes!("../../../reference/tinyllamas-260k/tok512.bin").to_vec(),
        sources,
    )
    .unwrap();
    let corpus = Corpus::parse(include_bytes!(
        "../../../tests/fixtures/local-v0/corpus.json"
    ))
    .unwrap();
    let study = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/study-v0/profile.json"
    ))
    .unwrap();
    let initial = plan::apply(&m, &corpus, &study, "mixed-linear-pruned")
        .unwrap()
        .model;
    let mut cfg: adaptation::Config = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/adaptation-v0/config.json"
    ))
    .unwrap();
    cfg.optimizer.max_steps = 2;
    cfg.row_limit = 8;
    let start = adaptation::State::start(&m, &initial, corpus.clone(), cfg.clone()).unwrap();
    let mut full = start.clone();
    full.advance(&m, 2).unwrap();
    let mut partial = start;
    partial.advance(&m, 1).unwrap();
    let path = std::env::temp_dir().join(format!("nrl-adamw-{}.json", std::process::id()));
    partial.save(&m, &path).unwrap();
    let mut resumed = adaptation::State::load(&m, &path).unwrap();
    resumed.advance(&m, 2).unwrap();
    assert_eq!(
        serde_json::to_value(&full).unwrap(),
        serde_json::to_value(&resumed).unwrap()
    );
    let mut changed = corpus;
    for d in &mut changed.documents {
        if d.split == "test" {
            d.text.push_str(" The end.");
        }
    }
    let mut other = adaptation::State::start(&m, &initial, changed, cfg).unwrap();
    other.advance(&m, 2).unwrap();
    assert_eq!(full.best_step, other.best_step);
    assert_eq!(full.validation_nll, other.validation_nll);
    assert_eq!(
        serde_json::to_value(&full.current).unwrap(),
        serde_json::to_value(&other.current).unwrap()
    );
    let mut corrupt: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    corrupt["payload"]["optimizer"]["step"] = 0.into();
    std::fs::write(&path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
    assert!(adaptation::State::load(&m, &path).is_err());
    std::fs::remove_file(path).unwrap();
}
