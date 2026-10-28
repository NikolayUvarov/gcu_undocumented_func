use nrl_lab::{
    close,
    decoder::{self, Decoder},
    model::Source,
    qsl_model::{self, Binding},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fixture() -> PathBuf {
    root().join("tests/fixtures/qsl-model-v0")
}
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
fn binding() -> Binding {
    Binding::load(&fixture().join("binding.json")).unwrap()
}
fn directory() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "nrl-qsl-model-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn original_qsl_four_path_overlay_runs_the_complete_real_model_against_pytorch() {
    let m = model();
    let imported = qsl_model::import(&m, &fixture(), binding(), "MIT").unwrap();
    let oracle: Value = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/qsl-model-v0/expected.json"
    ))
    .unwrap();
    assert_eq!(imported.info().unwrap()["base_identity_verified"], true);
    assert_eq!(imported.model.patches().len(), 4);
    for case in oracle["cases"].as_array().unwrap() {
        let tokens: Vec<usize> = serde_json::from_value(case["tokens"].clone()).unwrap();
        let logits: Vec<f64> = serde_json::from_value(case["logits"].clone()).unwrap();
        close(&imported.model.forward(&tokens).unwrap(), &logits).unwrap();
        assert_eq!(
            serde_json::to_value(imported.model.generate(&tokens, 4).unwrap()).unwrap(),
            case["generated"]
        );
    }
    assert_eq!(m.patches().len(), 0);
    assert_eq!(m.model_id(), binding().base_model_id);
}
#[test]
fn overlay_roundtrip_and_standalone_remain_executable_without_source_or_base() {
    let dir = directory();
    let source = dir.join("source");
    fs::create_dir(&source).unwrap();
    for name in ["manifest.json", "blocks.safetensors"] {
        fs::copy(fixture().join(name), source.join(name)).unwrap();
    }
    let base = dir.join("base");
    model().save(&base).unwrap();
    let m = decoder::load(&base).unwrap();
    let imported = qsl_model::import(&m, &source, binding(), "MIT").unwrap();
    let expected = imported.model.forward(&[1, 259, 260]).unwrap();
    let overlay = dir.join("overlay");
    imported.save_overlay(&overlay).unwrap();
    let restored = qsl_model::load(&m, &overlay).unwrap();
    assert_eq!(restored.model.model_id(), imported.model.model_id());
    assert_eq!(restored.model.forward(&[1, 259, 260]).unwrap(), expected);
    let standalone = dir.join("standalone");
    restored.model.save(&standalone).unwrap();
    fs::remove_dir_all(&base).unwrap();
    fs::remove_dir_all(&source).unwrap();
    fs::remove_dir_all(&overlay).unwrap();
    drop(restored);
    drop(imported);
    drop(m);
    let loaded = decoder::load(&standalone).unwrap();
    assert_eq!(loaded.forward(&[1, 259, 260]).unwrap(), expected);
    assert!(loaded
        .sources()
        .iter()
        .any(|s| s.identifier.starts_with("qsl-overlay://")
            && s.notices.iter().any(|n| n.contains("fingerprint_policy"))));
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn wrong_base_mapping_scope_and_corrupted_sources_are_refused() {
    let m = model();
    for mode in 0..5 {
        let mut b = binding();
        match mode {
            0 => b.base_model_id = "0".repeat(64),
            1 => b.adapter = "qwen3".into(),
            2 => {
                b.layers.insert(0, 3);
            }
            3 => b.qsl_artifact_id = "f".repeat(64),
            _ => {
                b.layers.remove(&0);
            }
        }
        assert!(qsl_model::import(&m, &fixture(), b, "MIT").is_err());
    }
    let mut s = m.sources().to_vec();
    s[0].notices.push("Different exact base provenance".into());
    let other = Decoder::from_bytes(
        include_bytes!("../../../reference/tinyllamas-260k/stories260K.bin").to_vec(),
        include_bytes!("../../../reference/tinyllamas-260k/tok512.bin").to_vec(),
        s,
    )
    .unwrap();
    assert!(qsl_model::import(&other, &fixture(), binding(), "MIT").is_err());
    let dir = directory();
    let overlay = dir.join("overlay");
    qsl_model::import(&m, &fixture(), binding(), "MIT")
        .unwrap()
        .save_overlay(&overlay)
        .unwrap();
    let weights = overlay.join("qsl/blocks.safetensors");
    let mut raw = fs::read(&weights).unwrap();
    let last = raw.len() - 1;
    raw[last] ^= 1;
    fs::write(&weights, raw).unwrap();
    assert!(qsl_model::load(&m, &overlay).is_err());
    fs::remove_dir_all(dir).unwrap();
}
