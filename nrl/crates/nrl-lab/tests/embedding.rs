use nrl_lab::{
    close,
    embedding::{self, Definition, Encoder},
    model::{Model, ObjectData, Source},
    research::{
        evaluate,
        retrieval::{self, Dataset},
    },
};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
fn definition() -> Definition {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-v0/definition.json"
    ))
    .unwrap()
}
fn encoder() -> Encoder {
    Encoder::from_definition(definition()).unwrap()
}
fn data() -> Dataset {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-v0/data.json"
    ))
    .unwrap()
}
fn oracle() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-v0/expected.json"
    ))
    .unwrap()
}
fn source() -> Source {
    serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/embedding-v0/replacement-source.json"
    ))
    .unwrap()
}
fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "nrl-embedding-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn new_family_vectors_scores_rankings_and_metrics_match_pytorch() {
    let e = encoder();
    let d = data();
    let expected = oracle();
    for (text, v) in expected["vectors"].as_object().unwrap() {
        let reference: Vec<f64> = serde_json::from_value(v.clone()).unwrap();
        close(&e.encode(text).unwrap(), &reference).unwrap();
    }
    for split in ["train", "validation", "test"] {
        let r = retrieval::evaluate(&e, &e, &d, split, None).unwrap();
        let selected: Vec<_> = expected["queries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|q| q["split"] == split)
            .collect();
        let mut sum = 0.0;
        for (a, b) in r.quality.details["rankings"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&selected)
        {
            assert_eq!(a["query_id"], b["id"]);
            for (x, y) in a["ranking"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["ranking"].as_array().unwrap())
            {
                assert_eq!(x[0], y[0]);
                close(&[x[1].as_f64().unwrap()], &[y[1].as_f64().unwrap()]).unwrap();
            }
            for metric in ["recall_at_k", "reciprocal_rank", "ndcg_at_k"] {
                close(
                    &[a[metric].as_f64().unwrap()],
                    &[b[metric].as_f64().unwrap()],
                )
                .unwrap();
            }
            sum += b["ndcg_at_k"].as_f64().unwrap();
        }
        close(&[r.quality.value], &[1.0 - sum / selected.len() as f64]).unwrap();
        assert_eq!(r.profile.task, "embedding-retrieval/0");
        let mut bad = r.clone();
        bad.quality.value = 1.01;
        assert!(bad.validate().is_err());
        assert_eq!(r.work.rows_per_pass, 8);
        assert!(!r.native_execution);
    }
}
#[test]
fn projection_reuses_all_five_representations_with_checked_external_boundary() {
    let e = encoder();
    let d = definition();
    let original = match &d.projection.objects["body"] {
        ObjectData::Gated(a) => a.clone(),
        _ => panic!(),
    };
    let p = nrl_lab::prune(&original, 2, None).unwrap();
    let svd = nrl_lab::svd::control(&original, 1).unwrap();
    let controls = vec![
        original,
        p.clone(),
        nrl_lab::linear_spline_control(&p, 4, vec![-1.; 2], vec![1.; 2]).unwrap(),
        nrl_lab::cubic_spline_control(&p, 4, vec![-1.; 2], vec![1.; 2]).unwrap(),
        svd,
    ];
    for block in controls {
        let mut projection = d.projection.clone();
        projection
            .objects
            .insert("body".into(), ObjectData::Gated(block));
        let p = Model::from_definition(projection).unwrap();
        let candidate = e.replace_projection(p, source()).unwrap();
        assert_eq!(
            candidate.input_content_id().unwrap(),
            e.input_content_id().unwrap()
        );
        assert_ne!(candidate.model_id(), e.model_id());
        assert_eq!(candidate.encode("fox").unwrap().len(), 3);
        let a = retrieval::evaluate(&e, &e, &data(), "validation", None).unwrap();
        let b = retrieval::evaluate(&e, &candidate, &data(), "validation", None).unwrap();
        assert!(evaluate::compare(&a, &b).is_ok());
    }
}
#[test]
fn standalone_roundtrip_preserves_sources_history_and_refuses_corrupt_packages() {
    let e = encoder();
    let dir = directory();
    let path = dir.join("model");
    e.save(&path).unwrap();
    let loaded = embedding::load(&path).unwrap();
    assert_eq!(loaded.model_id(), e.model_id());
    assert_eq!(
        loaded.encode("moon sky").unwrap(),
        e.encode("moon sky").unwrap()
    );
    assert!(e.save(&path).is_err());
    let candidate = loaded
        .replace_projection(
            Model::from_definition(definition().projection).unwrap(),
            source(),
        )
        .unwrap();
    let other = dir.join("changed");
    candidate.save(&other).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(other.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["previous_projection_ids"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(manifest["sources"].as_array().unwrap().len() >= 3);
    let table = path.join("table.json");
    let mut bytes = fs::read(&table).unwrap();
    bytes.push(b' ');
    fs::write(&table, bytes).unwrap();
    assert!(embedding::load(&path).is_err());
    let vector = dir.join("vector.json");
    e.encode_to_file("fox", &vector).unwrap();
    assert!(e.encode_to_file("fox", &vector).is_err());
    let mut unknown = serde_json::to_value(definition()).unwrap();
    unknown["extra"] = json!(true);
    let file = dir.join("unknown.json");
    fs::write(&file, serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert!(Definition::load(&file).is_err());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn input_semantics_split_leakage_zero_norm_and_resource_bounds_fail_clearly() {
    let e = encoder();
    assert!(e.encode("FOX").is_err());
    assert!(e.encode("🦊").is_err());
    assert!(e.encode(" ").is_err());
    assert!(e.encode_tokens(&[]).is_err());
    assert!(e.encode_tokens(&[usize::MAX]).is_err());
    assert!(e.encode_tokens(&vec![0; 129]).is_err());
    let mut d = definition();
    d.table.values[0] = f64::NAN;
    assert!(Encoder::from_definition(d).is_err());
    let mut d = definition();
    if let ObjectData::Affine(a) = d.projection.objects.get_mut("readout").unwrap() {
        a.weights.fill(0.0);
        a.bias.fill(0.0);
    }
    assert!(Encoder::from_definition(d).unwrap().encode("fox").is_err());
    let mut changed = definition();
    changed.table.values[0] += 0.1;
    let changed = Encoder::from_definition(changed).unwrap();
    assert!(retrieval::evaluate(&e, &changed, &data(), "test", None).is_err());
    let mut ds = data();
    ds.queries[4].group = ds.queries[0].group.clone();
    assert!(ds.validate().is_err());
    let mut ds = data();
    ds.queries[0].text = "fox dog".into();
    ds.queries[4].text = "dog fox".into();
    assert!(ds.tokenize(&e).is_err());
    let mut ds = data();
    ds.queries[0].relevance.insert("missing".into(), 3);
    assert!(ds.validate().is_err());
    let mut spec: evaluate::Measurement = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/study-v0/measurement.json"
    ))
    .unwrap();
    spec.max_forward_rows = 1;
    assert!(retrieval::evaluate(&e, &e, &data(), "test", Some(spec)).is_err());
}

#[test]
fn independent_embedding_cli_roundtrip_prune_benchmark_and_compare() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = directory();
    let exe = env!("CARGO_BIN_EXE_nrl-lab");
    let run = |args: Vec<String>| {
        let output = std::process::Command::new(exe).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let base = dir.join("base");
    let pruned = dir.join("pruned");
    let definition = root.join("tests/fixtures/embedding-v0/definition.json");
    let source = root.join("tests/fixtures/embedding-v0/replacement-source.json");
    let data = root.join("tests/fixtures/embedding-v0/data.json");
    let spec = root.join("tests/fixtures/study-v0/measurement.json");
    let text = |p: &std::path::Path| p.to_str().unwrap().to_owned();
    run(vec![
        "import-embedding".into(),
        text(&definition),
        text(&base),
    ]);
    run(vec![
        "prune-embedding".into(),
        text(&base),
        "body".into(),
        "2".into(),
        text(&source),
        text(&pruned),
    ]);
    let a = dir.join("baseline.json");
    let b = dir.join("candidate.json");
    for (model, out) in [(&base, &a), (&pruned, &b)] {
        run(vec![
            "benchmark-retrieval".into(),
            text(&base),
            text(model),
            text(&data),
            "test".into(),
            text(&spec),
            text(out),
        ]);
    }
    let output = dir.join("comparison.json");
    run(vec![
        "compare-reports".into(),
        text(&a),
        text(&b),
        text(&output),
    ]);
    let comparison: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(comparison["metric"], "ndcg-loss-at-k");
    assert!(comparison["quality_delta"].is_number());
    assert_ne!(
        embedding::load(&base).unwrap().model_id(),
        embedding::load(&pruned).unwrap().model_id()
    );
    fs::remove_dir_all(dir).unwrap();
}
