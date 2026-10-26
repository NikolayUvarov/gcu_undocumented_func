use nrl_lab::{close, qsl, safetensors};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const MANIFEST: &[u8] = include_bytes!("../../../tests/fixtures/qsl-overlay-v1/manifest.json");
const WEIGHTS: &[u8] = include_bytes!("../../../tests/fixtures/qsl-overlay-v1/blocks.safetensors");
fn expected() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/qsl-overlay-v1/expected.json"
    ))
    .unwrap()
}
fn vector(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "nrl-qsl-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn manifest(weights: &[u8]) -> Value {
    let mut m: Value = serde_json::from_slice(MANIFEST).unwrap();
    m["weights_sha256"] = Value::String(qsl::hash(weights));
    m["patch_file_bytes"] = json!(weights.len());
    resign(&mut m);
    m
}
fn resign(m: &mut Value) {
    m["artifact_id"] = Value::String(qsl::artifact_id(
        m["base"]["fingerprint"].as_str().unwrap(),
        &m["blocks"],
        m["weights_sha256"].as_str().unwrap(),
    ));
}
fn bytes(m: &Value) -> Vec<u8> {
    serde_json::to_vec(m).unwrap()
}
fn header() -> (Value, Vec<u8>) {
    let n = u64::from_le_bytes(WEIGHTS[..8].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&WEIGHTS[8..8 + n]).unwrap(),
        WEIGHTS[8 + n..].to_vec(),
    )
}
fn tensor_file(header: &Value, data: &[u8]) -> Vec<u8> {
    let mut header = bytes(header);
    while header.len() % 8 != 0 {
        header.push(b' ');
    }
    let mut out = (header.len() as u64).to_le_bytes().to_vec();
    out.extend(header);
    out.extend(data);
    out
}

#[test]
fn all_five_saved_qsl_representations_import_against_external_f64_oracle() {
    let set = qsl::import_bytes(MANIFEST, WEIGHTS, "MIT; synthetic fixture").unwrap();
    let oracle = expected();
    assert_eq!(
        set.manifest().source.weights_sha256,
        oracle["weights_sha256"].as_str().unwrap()
    );
    assert_eq!(
        set.manifest().source.artifact_id,
        oracle["artifact_id"].as_str().unwrap()
    );
    assert_eq!(set.manifest().scope, "standalone-numerical-blocks");
    assert!(!set.manifest().source.base_identity_verified);
    assert_eq!(set.manifest().source.manifest_sha256, qsl::hash(MANIFEST));
    assert_eq!(
        set.manifest().unrecorded_channel_mappings,
        ["qsl.layer.1", "qsl.layer.2", "qsl.layer.3"]
    );
    let input = vector(&oracle["input"]);
    let mut count = 0;
    for (id, block) in set.blocks() {
        close(
            &block.forward(&input).unwrap(),
            &vector(&oracle["outputs"][id]),
        )
        .unwrap();
        count += 1;
        if let Some(selection) = &block.selection {
            assert_eq!(selection.mapping, "unrecorded");
            assert!(selection.indices.is_empty());
            assert_eq!(selection.original_channels, 4);
        }
    }
    assert_eq!(count, 5);
}

#[test]
fn standalone_block_set_round_trips_and_never_overwrites() {
    let root = directory();
    let path = root.join("first");
    let second = root.join("second");
    let set = qsl::import_bytes(MANIFEST, WEIGHTS, "MIT; synthetic fixture").unwrap();
    set.save(&path).unwrap();
    let before = fs::read(path.join("manifest.json")).unwrap();
    assert!(set.save(&path).is_err());
    assert_eq!(fs::read(path.join("manifest.json")).unwrap(), before);
    let loaded = qsl::load(&path).unwrap();
    loaded.save(&second).unwrap();
    let reloaded = qsl::load(&second).unwrap();
    let input = vector(&expected()["input"]);
    for ((id, a), (other, b)) in set.blocks().zip(reloaded.blocks()) {
        assert_eq!(id, other);
        assert_eq!(a.forward(&input).unwrap(), b.forward(&input).unwrap());
    }
    assert_eq!(
        fs::read(second.join("source-manifest.json")).unwrap(),
        MANIFEST
    );
    assert_eq!(
        loaded.manifest().expanded_block_bytes,
        reloaded.manifest().expanded_block_bytes
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_package_paths_hashes_source_metadata_and_flags_are_refused() {
    let root = directory();
    let set = qsl::import_bytes(MANIFEST, WEIGHTS, "MIT").unwrap();
    let path = root.join("set");
    set.save(&path).unwrap();
    let raw = fs::read(path.join("manifest.json")).unwrap();
    let m: Value = serde_json::from_slice(&raw).unwrap();
    for (field, value) in [
        ("file", json!("../outside.json")),
        ("sha256", json!("0".repeat(64))),
        ("bytes", json!(1)),
    ] {
        let mut bad = m.clone();
        bad["blocks"][0][field] = value;
        fs::write(path.join("manifest.json"), bytes(&bad)).unwrap();
        assert!(qsl::load(&path).is_err());
    }
    let mut bad = m.clone();
    bad["source"]["base_identity_verified"] = json!(true);
    fs::write(path.join("manifest.json"), bytes(&bad)).unwrap();
    assert!(qsl::load(&path).is_err());
    let mut bad = m.clone();
    bad["unrecorded_channel_mappings"] = json!([]);
    fs::write(path.join("manifest.json"), bytes(&bad)).unwrap();
    assert!(qsl::load(&path).is_err());
    fs::write(path.join("manifest.json"), &raw).unwrap();
    fs::write(path.join("source-manifest.json"), b"{}").unwrap();
    assert!(qsl::load(&path).is_err());
    fs::write(path.join("source-manifest.json"), MANIFEST).unwrap();
    let block = path.join("block-0000.json");
    let mut corrupted = fs::read(&block).unwrap();
    corrupted.push(b' ');
    fs::write(&block, corrupted).unwrap();
    assert!(qsl::load(&path).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn input_identity_licence_limits_versions_and_duplicate_json_are_checked() {
    assert!(qsl::import_bytes(MANIFEST, WEIGHTS, " ").is_err());
    let mut corrupted = WEIGHTS.to_vec();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    assert!(qsl::import_bytes(MANIFEST, &corrupted, "MIT")
        .err()
        .unwrap()
        .contains("checksum"));
    let base: Value = serde_json::from_slice(MANIFEST).unwrap();
    for (field, value) in [
        ("artifact_id", json!("0".repeat(64))),
        ("patch_file_bytes", json!(WEIGHTS.len() + 1)),
        ("patch_storage_dtype", json!("float16")),
        ("format", json!("qwen-spline-lab-standalone-v1")),
    ] {
        let mut bad = base.clone();
        bad[field] = value;
        assert!(qsl::import_bytes(&bytes(&bad), WEIGHTS, "MIT").is_err());
    }
    let duplicate = br#"{"format":"qwen-spline-lab-overlay-v1","details":{"x":1,"x":2}}"#;
    assert!(qsl::import_bytes(duplicate, WEIGHTS, "MIT")
        .err()
        .unwrap()
        .contains("duplicate"));
    let mut bad = base.clone();
    bad["blocks"]["0"]["hidden_size"] = json!(0);
    assert!(qsl::import_bytes(&bytes(&bad), WEIGHTS, "MIT").is_err());
    let mut bad = base.clone();
    bad["blocks"]["0"]["hidden_size"] = json!(true);
    assert!(qsl::import_bytes(&bytes(&bad), WEIGHTS, "MIT").is_err());
    let mut bad = base.clone();
    bad["blocks"]["0"]["representation"] = json!("other");
    assert!(qsl::import_bytes(&bytes(&bad), WEIGHTS, "MIT").is_err());
    assert!(qsl::import_bytes(&vec![b' '; qsl::MAX_MANIFEST_BYTES + 1], WEIGHTS, "MIT").is_err());
    assert!(
        qsl::import_bytes(MANIFEST, &vec![0; safetensors::MAX_WEIGHT_BYTES + 1], "MIT").is_err()
    );
}

#[test]
fn tensor_dtype_shape_offsets_nonfinite_values_and_exact_inventory_are_checked() {
    let (original, data) = header();
    let name = "layer_000.gate_proj.weight";
    for (field, value) in [
        ("dtype", json!("F64")),
        ("shape", json!([0, 3])),
        ("shape", json!([usize::MAX, 3])),
        ("data_offsets", json!([0, usize::MAX])),
    ] {
        let mut bad = original.clone();
        bad[name][field] = value;
        assert!(safetensors::read(&tensor_file(&bad, &data)).is_err());
    }
    let mut overlap = original.clone();
    overlap[name]["data_offsets"] = original["layer_000.up_proj.weight"]["data_offsets"].clone();
    assert!(safetensors::read(&tensor_file(&overlap, &data))
        .err()
        .unwrap()
        .contains("overlapping"));
    let mut nan = data.clone();
    let offset = original[name]["data_offsets"][0].as_u64().unwrap() as usize;
    nan[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(safetensors::read(&tensor_file(&original, &nan)).is_err());
    let mut extra = data.clone();
    extra.push(0);
    assert!(safetensors::read(&tensor_file(&original, &extra)).is_err());
    let mut missing = original.clone();
    let tensor = missing.as_object_mut().unwrap().remove(name).unwrap();
    missing["layer_000.fail_proj.weight"] = tensor;
    let weights = tensor_file(&missing, &data);
    let m = manifest(&weights);
    assert!(qsl::import_bytes(&bytes(&m), &weights, "MIT")
        .err()
        .unwrap()
        .contains("missing QSL tensor"));
    let mut unused: Value = serde_json::from_slice(MANIFEST).unwrap();
    unused["blocks"].as_object_mut().unwrap().remove("0");
    resign(&mut unused);
    assert!(qsl::import_bytes(&bytes(&unused), WEIGHTS, "MIT")
        .err()
        .unwrap()
        .contains("unused"));
    let mut wrong_shape = original.clone();
    wrong_shape[name]["shape"] = json!([3, 4]);
    let weights = tensor_file(&wrong_shape, &data);
    let m = manifest(&weights);
    assert!(qsl::import_bytes(&bytes(&m), &weights, "MIT")
        .err()
        .unwrap()
        .contains("shape mismatch"));
    assert!(safetensors::read(&[255; 16]).is_err());
}

#[cfg(unix)]
#[test]
fn directory_import_refuses_symlinked_payloads() {
    use std::os::unix::fs::symlink;
    let root = directory();
    fs::write(root.join("manifest.json"), MANIFEST).unwrap();
    fs::write(root.join("actual.safetensors"), WEIGHTS).unwrap();
    symlink("actual.safetensors", root.join("blocks.safetensors")).unwrap();
    assert!(qsl::import_directory(&root, "MIT")
        .err()
        .unwrap()
        .contains("symlink"));
    fs::remove_file(root.join("blocks.safetensors")).unwrap();
    fs::write(root.join("blocks.safetensors"), WEIGHTS).unwrap();
    assert_eq!(
        qsl::import_directory(&root, "MIT")
            .unwrap()
            .blocks()
            .count(),
        5
    );
    fs::remove_dir_all(root).unwrap();
}
