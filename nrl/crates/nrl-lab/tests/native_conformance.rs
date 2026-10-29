use nrl_lab::{model, native_conformance as native};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nrl-native-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn bundle(&self) -> PathBuf {
        self.0.join("bundle")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/native-conformance-v0")
}
fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn pinned_bundle_has_independent_reference_and_reproducible_runtime_scenarios() {
    let committed = native::check(&fixture()).unwrap();
    let temp = Temp::new();
    native::make(&temp.bundle()).unwrap();
    assert_eq!(native::check(&temp.bundle()).unwrap(), committed);
    assert!(native::make(&temp.bundle()).is_err());
    assert_eq!(committed["native_execution"], false);
    let cases = committed["host_scenarios"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    let partial = cases
        .iter()
        .find(|c| c["case"] == "partial-cancel")
        .unwrap();
    assert_eq!(partial["rows"], 2);
    assert_eq!(partial["work_units"], 646);
    assert_eq!(partial["requests"], 1);
    let oracle = json(&fs::read(fixture().join("reference.json")).unwrap());
    let expected: Vec<f64> = fs::read(fixture().join("expected.f64le"))
        .unwrap()
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(
        serde_json::to_value(&expected).unwrap(),
        oracle["original_output"]
    );
    assert_eq!(oracle["torch"], "2.10.0+cpu");
    let index = json(&fs::read(fixture().join("bundle.json")).unwrap());
    assert_eq!(index["model_id"], native::MODEL_ID);
    assert_eq!(index["reference"]["absolute_tolerance"], 1e-12);
    assert_eq!(index["reference"]["relative_tolerance"], 1e-10);
}

#[test]
fn tampering_cannot_be_hidden_by_replacing_the_caller_supplied_inventory() {
    let temp = Temp::new();
    native::make(&temp.bundle()).unwrap();
    for path in [
        "input.f64le",
        "expected.f64le",
        "model/object-0000.json",
        "bundle.json",
    ] {
        let target = temp.bundle().join(path);
        let saved = fs::read(&target).unwrap();
        let mut changed = saved.clone();
        changed[0] ^= 1;
        fs::write(&target, changed).unwrap();
        assert!(native::check(&temp.bundle()).is_err(), "accepted {path}");
        fs::write(&target, saved).unwrap();
    }
    fs::write(temp.bundle().join("extra"), b"unlisted").unwrap();
    assert!(native::check(&temp.bundle()).is_err());
    fs::remove_file(temp.bundle().join("extra")).unwrap();
    assert!(native::check(&temp.bundle()).is_ok());
}

#[test]
fn encoding_vectors_preserve_float_bits_and_scope_the_canonical_writer_subset() {
    let bytes = fs::read(fixture().join("object-encoding.json")).unwrap();
    native::check_object_encoding(&bytes).unwrap();
    let object: model::ObjectData = serde_json::from_slice(&bytes).unwrap();
    let model::ObjectData::Affine(affine) = object else {
        panic!("expected affine")
    };
    let bits = json(&fs::read(fixture().join("encoding-bits.json")).unwrap());
    let actual: Vec<_> = affine
        .weights
        .iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        bits["weights_f64_bits_hex"]
    );
    let text = std::str::from_utf8(&bytes).unwrap();
    for number in ["-0.0", "1.0", "5e-324", "1e+20", "1e-20", "0.00001", "1e-6"] {
        assert!(
            text.contains(number),
            "missing golden representation {number}"
        );
    }
    let compact = serde_json::to_vec(&affine).unwrap();
    assert!(native::check_object_encoding(&compact).is_err());
    let compact = serde_json::to_vec(&json(&bytes)).unwrap();
    assert!(native::check_object_encoding(&compact).is_err());
    assert!(
        native::check_object_encoding(br#"{"kind":"affine","kind":"affine","data":{}}"#).is_err()
    );
    let temp = Temp::new();
    native::make(&temp.bundle()).unwrap();
    let path = temp.bundle().join("model");
    let mut manifest = json(&fs::read(path.join("manifest.json")).unwrap());
    let record = manifest["objects"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    let object_path = path.join(record["file"].as_str().unwrap());
    let old = fs::read(&object_path).unwrap();
    let compact = serde_json::to_vec(&json(&old)).unwrap();
    native::check_object_encoding(&old).unwrap();
    assert!(native::check_object_encoding(&compact).is_err());
    record["bytes"] = compact.len().into();
    record["sha256"] = nrl_lab::qsl::hash(&compact).into();
    fs::write(&object_path, compact).unwrap();
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(model::load(&path, None).is_ok());
    assert!(native::check(&temp.bundle()).is_err());
}

#[cfg(unix)]
#[test]
fn fixture_checker_refuses_symlink_payloads_and_directories() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    native::make(&temp.bundle()).unwrap();
    let payload = temp.bundle().join("input.f64le");
    let moved = temp.0.join("original-input");
    fs::rename(&payload, &moved).unwrap();
    symlink(&moved, &payload).unwrap();
    assert!(native::check(&temp.bundle()).is_err());
    fs::remove_file(payload).unwrap();
    fs::rename(moved, temp.bundle().join("input.f64le")).unwrap();
    let alias = temp.0.join("alias");
    symlink(temp.bundle(), &alias).unwrap();
    assert!(native::check(&alias).is_err());
}
