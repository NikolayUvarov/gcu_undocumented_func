use nrl_lab::{
    close,
    decoder::Decoder,
    model::Source,
    research::{
        evaluate::{self, Measurement, Report},
        Corpus,
    },
};
use serde_json::{json, Value};
use std::{fs, path::Path};
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

fn spec() -> Measurement {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/study-v0/measurement.json"
    ))
    .unwrap()
}
#[test]
fn language_metrics_match_independent_pytorch_full_vocabulary_reference() {
    let m = model();
    let c = reference_corpus();
    let expected: Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/evaluate-v0.json")).unwrap();
    for (i, split) in ["train", "validation", "test"].into_iter().enumerate() {
        let r = evaluate::evaluate_language(&m, &m, &c, split, None).unwrap();
        let e = &expected["cases"][i];
        close(
            &[
                r.quality.value,
                r.quality.details["perplexity"].as_f64().unwrap(),
                r.quality.details["top1_accuracy"].as_f64().unwrap(),
            ],
            &[
                e["mean_nll"].as_f64().unwrap(),
                e["perplexity"].as_f64().unwrap(),
                e["top1_accuracy"].as_f64().unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(r.work.output_values_per_pass, r.work.rows_per_pass * 512);
        assert_eq!(r.work.total_forward_rows, r.work.rows_per_pass);
    }
}
#[test]
fn regression_matches_the_existing_independent_training_profile() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reports/regression-example-2026-10-26");
    let a = nrl_lab::model::load(&root.join("initial-model"), None).unwrap();
    let b = nrl_lab::model::load(&root.join("selected-model"), None).unwrap();
    let data = nrl_lab::experiment::Dataset::parse(include_bytes!(
        "../../../tests/fixtures/training-v0/data.json"
    ))
    .unwrap();
    let ar = evaluate::evaluate_regression(&a, &a, &data, "test", None).unwrap();
    let br = evaluate::evaluate_regression(&a, &b, &data, "test", None).unwrap();
    close(
        &[ar.quality.value, br.quality.value],
        &[0.7019184889255535, 0.016083866958995992],
    )
    .unwrap();
    assert!(
        evaluate::compare(&ar, &br).unwrap()["quality_delta"]
            .as_f64()
            .unwrap()
            < 0.0
    );
}
#[test]
fn measured_work_distribution_and_comparability_are_checked() {
    let m = model();
    let c = reference_corpus();
    let r = evaluate::evaluate_language(&m, &m, &c, "validation", Some(spec())).unwrap();
    assert_eq!(r.work.total_forward_rows, r.work.rows_per_pass * 5);
    assert_eq!(r.timing.as_ref().unwrap().seconds.len(), 3);
    assert!(evaluate::compare(&r, &r).is_ok());
    for key in [
        "data", "split", "exe", "boot", "samples", "geometry", "total", "overflow", "negative",
        "native",
    ] {
        let mut v = serde_json::to_value(&r).unwrap();
        match key {
            "data" => v["profile"]["data_id"] = json!("a".repeat(64)),
            "split" => v["profile"]["split"] = json!("test"),
            "exe" => v["environment"]["executable_sha256"] = json!("a".repeat(64)),
            "boot" => v["environment"]["boot_sha256"] = json!("a".repeat(64)),
            "samples" => v["timing"]["seconds"][0] = json!(123.0),
            "geometry" => v["profile"]["geometry"] = json!([]),
            "total" => v["work"]["total_forward_rows"] = json!(0),
            "overflow" => v["work"]["rows_per_pass"] = json!(usize::MAX),
            "negative" => v["quality"]["value"] = json!(-1),
            _ => v["native_execution"] = json!(true),
        }
        let bad: Report = serde_json::from_value(v).unwrap();
        assert!(evaluate::compare(&r, &bad).is_err(), "{key}");
    }
    let mut tiny = spec();
    tiny.max_forward_rows = 1;
    assert!(evaluate::evaluate_language(&m, &m, &c, "validation", Some(tiny)).is_err());
    let tmp = std::env::temp_dir().join(format!("nrl-evaluation-{}.json", std::process::id()));
    r.save(&tmp).unwrap();
    assert!(r.save(&tmp).is_err());
    assert_eq!(
        Report::load(&tmp).unwrap().report_id().unwrap(),
        r.report_id().unwrap()
    );
    fs::remove_file(tmp).unwrap();
}
