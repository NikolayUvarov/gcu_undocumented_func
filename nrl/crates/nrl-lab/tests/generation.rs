use nrl_lab::{
    decoder::{self, Decoder},
    model::Source,
    research::generation,
};
use serde_json::Value;
fn model() -> Decoder {
    let v: Value = serde_json::from_slice(include_bytes!(
        "../../../reference/tinyllamas-260k/decoder-source.json"
    ))
    .unwrap();
    let s: Vec<Source> = serde_json::from_value(v["sources"].clone()).unwrap();
    Decoder::from_bytes(
        include_bytes!("../../../reference/tinyllamas-260k/stories260K.bin").to_vec(),
        include_bytes!("../../../reference/tinyllamas-260k/tok512.bin").to_vec(),
        s,
    )
    .unwrap()
}
fn check(m: &Decoder) {
    let tokens = [1, 403, 407, 269, 261, 280];
    let mut reference = m.state();
    let mut prepared = m.prepared_session().unwrap();
    for (i, t) in tokens.into_iter().enumerate() {
        assert_eq!(
            m.step(&mut reference, t).unwrap(),
            prepared.step(t).unwrap()
        );
        assert_eq!(prepared.position(), i + 1);
    }
    assert!(prepared.step(m.config().vocab).is_err());
    assert_eq!(prepared.position(), 6);
    assert_eq!(
        m.step(&mut reference, 1).unwrap(),
        prepared.step(1).unwrap()
    );
    for maximum in [0, 1, 8, 32, 64] {
        assert_eq!(
            m.generate_reference(&tokens, maximum).unwrap(),
            m.generate_prepared(&tokens, maximum).unwrap()
        );
    }
    for (prompt, n) in [
        (vec![], 1),
        (vec![m.config().vocab], 2),
        (vec![1], 65),
        (vec![1; m.config().context], 1),
    ] {
        assert!(m.generate_prepared(&prompt, n).is_err());
    }
}
#[test]
fn prepared_steps_logits_and_greedy_ids_match_dense_and_all_compressed_paths() {
    check(&model());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reports/plan-example-2026-10-26");
    for name in [
        "pruned-layer0",
        "spline-linear-layer0",
        "spline-cubic-layer0",
        "svd-layer0",
        "mixed-linear-pruned",
        "mixed-svd-cubic",
    ] {
        check(&decoder::load(&root.join(format!("{name}-model"))).unwrap());
    }
}
#[test]
fn prepared_sessions_roll_back_numerical_failures_and_keep_sessions_independent() {
    let m = model();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reports/plan-example-2026-10-26/pruned-layer0-model");
    let mut patches = decoder::load(&root).unwrap().patches();
    patches.get_mut(&0).unwrap().down.fill(f64::MAX);
    let bad = m.with_patches(patches, &[]).unwrap();
    let mut session = bad.prepared_session().unwrap();
    for _ in 0..2 {
        assert!(session.step(1).is_err());
        assert_eq!(session.position(), 0);
    }
    let first = m.trace(&[1]).unwrap().layers[0].input[0];
    let mut late = decoder::load(&root).unwrap().patches();
    let block = late.get_mut(&0).unwrap();
    block.gate.fill(0.0);
    block.up.fill(0.0);
    block.down.fill(0.0);
    block.gate[0] = 1.0;
    block.up[0] = 1e200;
    block.down[0] = f64::MAX;
    let mut bias = vec![0.0; block.shape[1]];
    bias[0] = -first;
    block.bias = Some(nrl_lab::BiasData {
        gate: bias,
        up: Vec::new(),
        down: Vec::new(),
    });
    let late = m.with_patches(late, &[]).unwrap();
    let mut reference = late.state();
    let mut prepared = late.prepared_session().unwrap();
    assert_eq!(
        late.step(&mut reference, 1).unwrap(),
        prepared.step(1).unwrap()
    );
    for _ in 0..2 {
        assert!(late.step(&mut reference, 259).is_err());
        assert!(prepared.step(259).is_err());
        assert_eq!(prepared.position(), 1);
        assert_eq!(reference.position(), 1);
    }
    let mut a = m.prepared_session().unwrap();
    let mut b = m.prepared_session().unwrap();
    assert_eq!(a.step(1).unwrap(), b.step(1).unwrap());
    assert_eq!(a.position(), 1);
    for _ in 1..m.config().context {
        a.step(1).unwrap();
    }
    assert!(a.step(1).is_err());
    assert_eq!(a.position(), m.config().context);
    assert_eq!(b.position(), 1);
}
#[test]
fn fixed_profile_records_pairs_and_refuses_excess_work() {
    let m = model();
    let s: generation::Study = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/generation-v0/study.json"
    ))
    .unwrap();
    let r = generation::profile(&m, &s).unwrap();
    assert_eq!(r["identical_tokens"], true);
    assert_eq!(r["reference"]["seconds"].as_array().unwrap().len(), 7);
    let mut bad = s;
    bad.measurement.max_forward_rows = 1;
    assert!(generation::profile(&m, &bad).is_err());
}
