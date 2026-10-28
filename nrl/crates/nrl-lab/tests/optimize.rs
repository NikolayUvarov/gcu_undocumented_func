use nrl_lab::{
    close,
    decoder::{self, Decoder},
    model::Source,
    research::{
        optimize::{self, Study},
        Corpus,
    },
};
use serde_json::Value;
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
        "../../../tests/fixtures/optimization-v0/study.json"
    ))
    .unwrap()
}
#[test]
fn prepared_dense_logits_and_traces_match_independent_reference_and_original_path() {
    let m = model();
    let oracle: Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/decoder-v0.json")).unwrap();
    for case in oracle["cases"].as_array().unwrap().iter().take(3) {
        let t: Vec<usize> = serde_json::from_value(case["tokens"].clone()).unwrap();
        let expected: Vec<f64> = serde_json::from_value(case["f64_logits"].clone()).unwrap();
        close(&m.forward_prepared(&t).unwrap(), &expected).unwrap();
        let a = m.trace(&t).unwrap();
        let b = m.trace_prepared(&t).unwrap();
        assert_eq!(a.logits, b.logits);
        for (x, y) in a.layers.iter().zip(b.layers) {
            assert_eq!(x.input, y.input);
            assert_eq!(x.output, y.output);
        }
    }
    assert!(m.forward_prepared(&[]).is_err());
    assert!(m.forward_prepared(&[m.config().vocab]).is_err());
    assert!(m
        .forward_prepared(&vec![1; m.config().context + 1])
        .is_err());
    let t = vec![1; 128];
    assert_eq!(
        m.forward_reference(&t).unwrap(),
        m.forward_prepared(&t).unwrap()
    );
}
#[test]
fn all_four_compressed_controls_and_both_mixtures_preserve_scratch_reuse_results() {
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
        let m = decoder::load(&root.join(format!("{name}-model"))).unwrap();
        let tokens = vec![1, 403, 407, 269, 261, 280];
        assert_eq!(
            m.forward_reference(&tokens).unwrap(),
            m.forward_prepared(&tokens).unwrap(),
            "{name}"
        );
    }
}
#[test]
fn optimization_profile_counts_work_keeps_quality_and_refuses_other_studies() {
    let m = model();
    let c = corpus();
    let s = study();
    let r = optimize::profile(&m, &c, &s).unwrap();
    assert_eq!(r["quality_pass"], true);
    assert_eq!(r["reference"]["seconds"].as_array().unwrap().len(), 7);
    assert_eq!(r["prepared"]["seconds"].as_array().unwrap().len(), 7);
    assert_eq!(r["native_execution"], false);
    for q in r["quality"].as_array().unwrap() {
        assert_eq!(q["maximum_absolute_logit_difference"], 0.0);
        assert_eq!(q["argmax_changes"], 0);
    }
    let mut bad = s.clone();
    bad.measurement.max_forward_rows = 1;
    assert!(optimize::profile(&m, &c, &bad).is_err());
    let mut bad = s.clone();
    bad.model_id = "a".repeat(64);
    assert!(optimize::profile(&m, &c, &bad).is_err());
    let mut bad = s.clone();
    bad.calibration_rows = usize::MAX;
    assert!(bad.validate().is_err());
}
