use nrl_lab::{
    close,
    decoder::{self, Decoder, Tokenizer},
    model::Source,
    Artifact, SelectionData,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};
const WEIGHTS: &[u8] = include_bytes!("../../../reference/tinyllamas-260k/stories260K.bin");
const TOKENS: &[u8] = include_bytes!("../../../reference/tinyllamas-260k/tok512.bin");
fn sources() -> Vec<Source> {
    serde_json::from_value(
        serde_json::from_slice::<Value>(include_bytes!(
            "../../../reference/tinyllamas-260k/decoder-source.json"
        ))
        .unwrap()["sources"]
            .clone(),
    )
    .unwrap()
}
fn model() -> Decoder {
    Decoder::from_bytes(WEIGHTS.to_vec(), TOKENS.to_vec(), sources()).unwrap()
}
fn oracle() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/decoder-v0.json")).unwrap()
    })
}
fn values(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn ids(v: &Value) -> Vec<usize> {
    serde_json::from_value(v.clone()).unwrap()
}
fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "nrl-decoder-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn patch(model: &Decoder) -> Artifact {
    let d = model.dense(0).unwrap();
    let width = 4;
    Artifact {
        descriptor: d.descriptor(),
        schema: "nrl.block/5".into(),
        representation: "pruned".into(),
        shape: [d.dim, width, d.dim],
        bias: None,
        gate: d.gate[..width * d.dim].to_vec(),
        up: d.up[..width * d.dim].to_vec(),
        down: (0..d.dim)
            .flat_map(|o| d.down[o * d.hidden..o * d.hidden + width].iter().copied())
            .collect(),
        spline: None,
        factors: None,
        selection: Some(SelectionData {
            mapping: "unrecorded".into(),
            original_channels: d.hidden,
            indices: vec![],
            method: "qsl-unrecorded/0".into(),
        }),
    }
}
#[test]
fn actual_trained_model_matches_full_vectorized_torch_and_original_c_policy() {
    let model = model();
    assert_eq!(model.config().dim, 64);
    assert_eq!(model.config().layers, 5);
    assert_eq!(model.config().heads, 8);
    assert_eq!(model.config().kv_heads, 4);
    assert!(model.config().tied_head);
    let info = model.info().unwrap();
    assert_eq!(info["base_parameter_values"], 260032);
    assert_eq!(info["effective_parameter_values"], 260032);
    assert_eq!(info["effective_numeric_values"], 264128);
    for case in oracle()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c.get("f64_logits").is_some())
    {
        let input = ids(&case["tokens"]);
        let trace = model.trace(&input).unwrap();
        close(&trace.logits, &values(&case["f64_logits"])).unwrap();
        for (layer, reference) in trace.layers.iter().zip(case["layers"].as_array().unwrap()) {
            close(&layer.input, &values(&reference["input"])).unwrap();
            close(&layer.output, &values(&reference["output"])).unwrap();
        }
        for (a, b) in trace
            .logits
            .iter()
            .zip(values(&case["original_c_f32_logits"]))
        {
            assert!(
                (a - b).abs() <= 3e-4 + 3e-5 * b.abs(),
                "f64/original-f32 {a} != {b}"
            );
        }
    }
}
#[test]
fn tokenizer_matches_original_byte_bpe_utf8_whitespace_and_fallback_goldens() {
    let model = model();
    for case in oracle()["cases"].as_array().unwrap() {
        let encoded = model
            .tokenizer
            .encode(case["text"].as_str().unwrap(), true, false)
            .unwrap();
        assert_eq!(encoded, ids(&case["tokens"]));
        let raw = model.tokenizer.decode_bytes(&encoded).unwrap();
        let hex: String = raw.iter().map(|v| format!("{v:02x}")).collect();
        assert_eq!(hex, case["decoded_hex"]);
        assert_eq!(
            model.tokenizer.decode(&encoded).unwrap(),
            String::from_utf8(raw).unwrap()
        );
    }
    assert_eq!(model.tokenizer.encode("", true, true).unwrap(), vec![1, 2]);
    assert!(model.tokenizer.encode("a\0b", true, false).is_err());
    assert!(model
        .tokenizer
        .encode(&"a".repeat(4097), true, false)
        .is_err());
    assert!(model.tokenizer.decode(&[512]).is_err());
    assert!(model.tokenizer.decode(&[3 + 0xff]).is_err());
    let mut bad = TOKENS.to_vec();
    bad.extend_from_slice(&[0]);
    assert!(Tokenizer::parse(&bad, 512).is_err());
    let mut bad = TOKENS.to_vec();
    bad[4..8].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(Tokenizer::parse(&bad, 512).is_err());
    let mut bad = TOKENS.to_vec();
    bad[..4].copy_from_slice(&1000u32.to_le_bytes());
    assert!(Tokenizer::parse(&bad, 512).is_err());
}
#[test]
fn cached_steps_uncached_prefixes_state_binding_and_context_budgets_are_explicit() {
    let model = model();
    let input = ids(&oracle()["cases"][0]["tokens"]);
    let full = model.forward(&input).unwrap();
    let mut state = model.state();
    for (position, token) in input.iter().enumerate() {
        let cached = model.step(&mut state, *token).unwrap();
        assert_eq!(state.position(), position + 1);
        assert_eq!(cached, full[position * 512..(position + 1) * 512]);
        let prefix = model.forward(&input[..position + 1]).unwrap();
        assert_eq!(cached, prefix[prefix.len() - 512..]);
    }
    let position = state.position();
    assert!(model.step(&mut state, 512).is_err());
    assert_eq!(state.position(), position);
    let mut altered = sources();
    altered[0].licence.push_str("; different declaration");
    let other = Decoder::from_bytes(WEIGHTS.to_vec(), TOKENS.to_vec(), altered).unwrap();
    assert_ne!(other.model_id(), model.model_id());
    assert!(other.step(&mut state, 1).is_err());
    assert_eq!(state.position(), position);
    assert!(model.forward(&[]).is_err());
    assert!(model.forward(&vec![1; 513]).is_err());
    assert!(model.trace_with_limit(&input, 1).is_err());
    assert!(model.trace_with_limit(&input, (1 << 20) + 1).is_err());
    assert!(model.generate(&input, 65).is_err());
    assert!(model.generate(&vec![1; 510], 8).is_err());
    let base = 28 + 260032 * 4;
    let mut short = WEIGHTS[..base].to_vec();
    short[24..28].copy_from_slice(&1i32.to_le_bytes());
    short.extend_from_slice(&WEIGHTS[base..base + 16]);
    short.extend_from_slice(&WEIGHTS[base + 8192..base + 8192 + 16]);
    let short = Decoder::from_bytes(short, TOKENS.to_vec(), sources()).unwrap();
    let mut state = short.state();
    short.step(&mut state, 1).unwrap();
    assert!(short.step(&mut state, 1).is_err());
    assert_eq!(state.position(), 1);
}
#[test]
fn bounded_data_only_packages_and_layer_patches_round_trip_without_overwriting() {
    let root = directory();
    let model = model();
    let base = root.join("base");
    model.save(&base).unwrap();
    let loaded = decoder::load(&base).unwrap();
    assert_eq!(loaded.model_id(), model.model_id());
    assert!(loaded.save(&base).is_err());
    let patched = model
        .with_patches(BTreeMap::from([(0, patch(&model))]), &[])
        .unwrap();
    assert_ne!(patched.model_id(), model.model_id());
    assert_eq!(patched.sources(), model.sources());
    let mut old_state = model.state();
    assert!(patched.step(&mut old_state, 1).is_err());
    let path = root.join("patched");
    patched.save(&path).unwrap();
    let restored = decoder::load(&path).unwrap();
    assert_eq!(restored.model_id(), patched.model_id());
    assert_eq!(
        restored.forward(&[1, 259]).unwrap(),
        patched.forward(&[1, 259]).unwrap()
    );
    assert!(
        patched.info().unwrap()["effective_parameter_values"]
            .as_u64()
            .unwrap()
            < 260032
    );
    assert!(
        patched.info().unwrap()["package_bytes"].as_u64().unwrap()
            > model.info().unwrap()["package_bytes"].as_u64().unwrap()
    );
    assert!(model
        .with_patches(BTreeMap::from([(99, patch(&model))]), &[])
        .is_err());
    let mut header: Value =
        serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
    header["weights"]["file"] = json!("../weights.bin");
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec(&header).unwrap(),
    )
    .unwrap();
    assert!(decoder::load(&path).is_err());
    header["weights"]["file"] = json!("weights.bin");
    header["config"]["heads"] = json!(4);
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec(&header).unwrap(),
    )
    .unwrap();
    assert!(decoder::load(&path).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn headers_hashes_finiteness_source_pins_and_existing_outputs_are_checked() {
    for index in [0, 1, 2, 3, 4, 5, 6] {
        let mut bad = WEIGHTS.to_vec();
        bad[index * 4..index * 4 + 4].copy_from_slice(&0i32.to_le_bytes());
        assert!(Decoder::from_bytes(bad, TOKENS.to_vec(), sources()).is_err());
    }
    let mut bad = WEIGHTS.to_vec();
    bad[28..32].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(Decoder::from_bytes(bad, TOKENS.to_vec(), sources()).is_err());
    let mut bad = WEIGHTS.to_vec();
    bad.push(0);
    assert!(Decoder::from_bytes(bad, TOKENS.to_vec(), sources()).is_err());
    assert!(Decoder::from_bytes(
        WEIGHTS[..WEIGHTS.len() - 4].to_vec(),
        TOKENS.to_vec(),
        sources()
    )
    .is_err());
    let root = directory();
    let path = root.join("package");
    let model = model();
    model.save(&path).unwrap();
    let mut weights = fs::read(path.join("weights.bin")).unwrap();
    weights[100] ^= 1;
    fs::write(path.join("weights.bin"), weights).unwrap();
    assert!(decoder::load(&path).is_err());
    let output = root.join("prediction.json");
    model.probe_to_file("A red fox.", &output).unwrap();
    let before = fs::read(&output).unwrap();
    assert!(model.probe_to_file("Hi", &output).is_err());
    assert_eq!(before, fs::read(&output).unwrap());
    let reference =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/tinyllamas-260k");
    let mut source: Value =
        serde_json::from_slice(&fs::read(reference.join("decoder-source.json")).unwrap()).unwrap();
    source["weights_sha256"] = json!("0".repeat(64));
    fs::write(
        root.join("source.json"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    assert!(decoder::import(
        &reference.join("stories260K.bin"),
        &reference.join("tok512.bin"),
        &root.join("source.json")
    )
    .err()
    .unwrap()
    .contains("pinned"));
    fs::remove_dir_all(root).unwrap();
}
