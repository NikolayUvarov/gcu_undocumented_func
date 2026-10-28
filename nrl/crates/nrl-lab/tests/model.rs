use nrl_lab::{
    close,
    model::{self, AffineData, Definition, Model, Replacements},
};
use nrl_repr::affine::{Affine, AffineGradients};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const DEFINITION: &[u8] = include_bytes!("../../../tests/fixtures/model-v0/definition.json");
const REPLACEMENTS: &[u8] = include_bytes!("../../../tests/fixtures/model-v0/replacements.json");
fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/model-v0/expected.json"
    ))
    .unwrap()
}
fn vector(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn base() -> Model {
    Model::from_definition(Definition::parse(DEFINITION).unwrap()).unwrap()
}
fn replacements() -> Replacements {
    serde_json::from_slice(REPLACEMENTS).unwrap()
}
fn directory() -> PathBuf {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "nrl-model-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn write_manifest(path: &std::path::Path, v: &Value) {
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec_pretty(v).unwrap(),
    )
    .unwrap();
}

#[test]
fn complete_graph_and_overlay_match_explicit_external_torch_networks() {
    let expected = oracle();
    let input = vector(&expected["input"]);
    let base = base();
    close(
        &base.forward(&input).unwrap(),
        &vector(&expected["original_output"]),
    )
    .unwrap();
    let overlay = base.overlay(replacements()).unwrap();
    close(
        &overlay.forward(&input).unwrap(),
        &vector(&expected["overlay_output"]),
    )
    .unwrap();
    assert_ne!(
        base.forward(&input).unwrap(),
        overlay.forward(&input).unwrap()
    );
    assert_eq!(overlay.manifest().objects.len(), 2);
    assert_eq!(overlay.manifest().changes.len(), 2);
    assert_eq!(overlay.manifest().descriptor, base.manifest().descriptor);
    assert_ne!(overlay.model_id(), base.model_id());
    let info = base.info().unwrap();
    assert_eq!(info["objects"], expected["object_count"]);
    assert_eq!(info["numeric_values"], expected["numeric_values"]);
    assert_eq!(info["trainable_values"], expected["trainable_values"]);
    assert_eq!(
        info["tied_objects"],
        json!([{"object":"embed","nodes":["left","right"]}])
    );
    let plain = Model::from_definition(
        Definition::parse(include_bytes!(
            "../../../tests/fixtures/model-v0/plain.json"
        ))
        .unwrap(),
    )
    .unwrap();
    close(
        &plain.forward(&vector(&expected["plain_input"])).unwrap(),
        &vector(&expected["plain_output"]),
    )
    .unwrap();
}

#[test]
fn overlay_resolves_only_against_exact_base_and_flattened_model_runs_without_base_files() {
    let root = directory();
    let base_dir = root.join("base");
    let patch_dir = root.join("overlay");
    let flat_dir = root.join("flat");
    let input = vector(&oracle()["input"]);
    let expected;
    {
        let source = base();
        source.save(&base_dir).unwrap();
        let loaded = model::load(&base_dir, None).unwrap();
        assert_eq!(source.model_id(), loaded.model_id());
        let overlay = loaded.overlay(replacements()).unwrap();
        overlay.save(&patch_dir).unwrap();
        assert_eq!(fs::read_dir(&patch_dir).unwrap().count(), 3);
        assert!(model::load(&patch_dir, None)
            .err()
            .unwrap()
            .contains("base"));
        let mut altered = Definition::parse(DEFINITION).unwrap();
        altered.sources[0].licence.push_str("; changed record");
        let wrong = Model::from_definition(altered).unwrap();
        assert_eq!(
            wrong.forward(&input).unwrap(),
            source.forward(&input).unwrap()
        );
        assert!(model::load(&patch_dir, Some(&wrong))
            .err()
            .unwrap()
            .contains("identity"));
        let resolved = model::load(&patch_dir, Some(&loaded)).unwrap();
        assert_eq!(resolved.model_id(), overlay.model_id());
        expected = resolved.forward(&input).unwrap();
        let flat = resolved.standalone().unwrap();
        flat.save(&flat_dir).unwrap();
        assert!(flat.manifest().base.is_none());
        assert_eq!(flat.manifest().history, resolved.manifest().changes);
        assert_eq!(flat.manifest().sources, resolved.manifest().sources);
        let info = resolved.info().unwrap();
        assert!(info["dependency_bytes"].as_u64().unwrap() > 0);
        assert!(
            info["total_required_package_bytes"].as_u64().unwrap()
                > info["package_bytes"].as_u64().unwrap()
        );
        assert!(resolved.overlay(replacements()).is_err());
    }
    fs::remove_dir_all(&base_dir).unwrap();
    fs::remove_dir_all(&patch_dir).unwrap();
    let independent = model::load(&flat_dir, None).unwrap();
    assert_eq!(independent.forward(&input).unwrap(), expected);
    assert_eq!(independent.info().unwrap()["dependency_bytes"], 0);
    assert!(independent
        .manifest()
        .sources
        .iter()
        .all(|s| !s.notices.is_empty()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn affine_primitive_matches_external_gradients_and_independent_differences() {
    let f = oracle();
    let f = &f["affine"];
    let a: AffineData = serde_json::from_value(f["data"].clone()).unwrap();
    let x = vector(&f["input"]);
    let dy = vector(&f["dy"]);
    let mut output = vec![0.0; dy.len()];
    let mut dx = vec![0.0; x.len()];
    let mut dw = vec![0.0; a.weights.len()];
    let mut db = vec![0.0; a.bias.len()];
    let op = a.view().unwrap();
    for row in 0..x.len() / 2 {
        op.forward(&x[row * 2..row * 2 + 2], &mut output[row * 3..row * 3 + 3])
            .unwrap();
        op.backward(
            &x[row * 2..row * 2 + 2],
            &dy[row * 3..row * 3 + 3],
            &mut dx[row * 2..row * 2 + 2],
            &mut AffineGradients {
                weights: &mut dw,
                bias: &mut db,
            },
        )
        .unwrap();
    }
    close(&output, &vector(&f["output"])).unwrap();
    close(&dx, &vector(&f["dx"])).unwrap();
    close(&dw, &vector(&f["dw"])).unwrap();
    close(&db, &vector(&f["db"])).unwrap();
    let objective = |a: &AffineData, x: &[f64]| {
        let op = a.view().unwrap();
        let mut sum = 0.0;
        for (row, input) in x.chunks_exact(2).enumerate() {
            let mut y = [0.0; 3];
            op.forward(input, &mut y).unwrap();
            sum += y
                .iter()
                .zip(&dy[row * 3..row * 3 + 3])
                .map(|(a, b)| a * b)
                .sum::<f64>();
        }
        sum
    };
    for parameter in 0..3 {
        let expected = match parameter {
            0 => &x,
            1 => &a.weights,
            _ => &a.bias,
        };
        let gradients = match parameter {
            0 => &dx,
            1 => &dw,
            _ => &db,
        };
        for i in 0..expected.len() {
            let mut plus = a.clone();
            let mut minus = a.clone();
            let mut xp = x.clone();
            let mut xm = x.clone();
            match parameter {
                0 => {
                    xp[i] += 1e-6;
                    xm[i] -= 1e-6
                }
                1 => {
                    plus.weights[i] += 1e-6;
                    minus.weights[i] -= 1e-6
                }
                _ => {
                    plus.bias[i] += 1e-6;
                    minus.bias[i] -= 1e-6
                }
            }
            assert!(
                ((objective(&plus, &xp) - objective(&minus, &xm)) / 2e-6 - gradients[i]).abs()
                    < 1e-8
            );
        }
    }
    assert!(Affine::new(0, 1, &[], &[]).is_err());
    assert!(Affine::new(1, 1, &[f64::NAN], &[]).is_err());
    assert!(op.forward(&x[..2], &mut []).is_err());
}

#[test]
fn graph_references_shapes_aliases_reachability_profiles_and_input_budgets_are_enforced() {
    let original: Value = serde_json::from_slice(DEFINITION).unwrap();
    for (path, value) in [
        ("family", json!("qwen")),
        ("adapter_version", json!(99)),
        ("preprocessor", json!("tokenizer")),
        ("state", json!("recurrent")),
        ("tied_objects", json!([])),
    ] {
        let mut bad = original.clone();
        bad["descriptor"][path] = value;
        let d = Definition::parse(&serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(Model::from_definition(d).is_err());
    }
    for (field, value) in [
        ("backend", json!("cuda")),
        ("dtype", json!("f32")),
        ("layout", json!("column-major")),
        ("max_batch_rows", json!(0)),
        ("max_activation_values", json!(1)),
    ] {
        let mut bad = original.clone();
        bad["execution"][field] = value;
        assert!(Model::from_definition(
            Definition::parse(&serde_json::to_vec(&bad).unwrap()).unwrap()
        )
        .is_err());
    }
    let mut wrong_add = original.clone();
    wrong_add["descriptor"]["configuration"]["nodes"][3]["op"] =
        json!({"operator":"affine","input":"right","object":"head"});
    assert!(Model::from_definition(
        Definition::parse(&serde_json::to_vec(&wrong_add).unwrap()).unwrap()
    )
    .err()
    .unwrap()
    .contains("add width"));
    let mut wrong_kind = original.clone();
    wrong_kind["descriptor"]["configuration"]["nodes"][0]["op"]["operator"] = json!("gated");
    assert!(Model::from_definition(
        Definition::parse(&serde_json::to_vec(&wrong_kind).unwrap()).unwrap()
    )
    .is_err());
    let mut forward = original.clone();
    forward["descriptor"]["configuration"]["nodes"][0]["op"]["input"] = json!("prediction");
    assert!(Model::from_definition(
        Definition::parse(&serde_json::to_vec(&forward).unwrap()).unwrap()
    )
    .is_err());
    let mut duplicate = original.clone();
    duplicate["descriptor"]["configuration"]["nodes"][1]["id"] = json!("left");
    assert!(Model::from_definition(
        Definition::parse(&serde_json::to_vec(&duplicate).unwrap()).unwrap()
    )
    .is_err());
    let mut unreachable = original.clone();
    unreachable["descriptor"]["configuration"]["output"] = json!("left");
    assert!(Model::from_definition(
        Definition::parse(&serde_json::to_vec(&unreachable).unwrap()).unwrap()
    )
    .is_err());
    let mut wrong = Definition::parse(DEFINITION).unwrap();
    wrong.objects.remove("head");
    assert!(Model::from_definition(wrong).is_err());
    let mut extra = Definition::parse(DEFINITION).unwrap();
    extra
        .objects
        .insert("unused".into(), extra.objects["head"].clone());
    assert!(Model::from_definition(extra).is_err());
    let mut small = Definition::parse(DEFINITION).unwrap();
    small.execution.max_batch_rows = 1;
    let limited = Model::from_definition(small).unwrap();
    assert!(limited.forward(&[0.0; 6]).is_err());
    let mut small = Definition::parse(DEFINITION).unwrap();
    small.execution.max_activation_values = 47;
    let limited = Model::from_definition(small).unwrap();
    assert!(limited.forward(&[0.0; 3]).is_ok());
    assert!(limited.forward(&[0.0; 6]).is_err());
    assert!(base().forward(&[]).is_err());
    assert!(base().forward(&[0.0; 2]).is_err());
    assert!(base().forward(&[f64::NAN; 3]).is_err());
    let mut unknown = original;
    unknown["descriptor"]["configuration"]["nodes"][0]["op"]["operator"] = json!("attention");
    assert!(Definition::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
}

#[test]
fn package_hashes_paths_versions_and_provenance_cannot_be_silently_changed() {
    let root = directory();
    let path = root.join("base");
    base().save(&path).unwrap();
    let raw = fs::read(path.join("manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&raw).unwrap();
    let key = manifest["objects"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    for (field, value) in [
        ("file", json!("../outside.json")),
        ("sha256", json!("0".repeat(64))),
        ("bytes", json!(1)),
    ] {
        let mut bad = manifest.clone();
        bad["objects"][key][field] = value;
        write_manifest(&path, &bad);
        assert!(model::load(&path, None).is_err());
    }
    fs::write(path.join("manifest.json"), &raw).unwrap();
    let entry = manifest["objects"][key]["file"].as_str().unwrap();
    let object = path.join(entry);
    let saved = fs::read(&object).unwrap();
    let mut changed = saved.clone();
    changed.push(b' ');
    fs::write(&object, &changed).unwrap();
    assert!(model::load(&path, None).is_err());
    fs::write(&object, &saved).unwrap();
    for schema in ["nrl.model/99", "nrl.block-set/0"] {
        fs::write(
            path.join("manifest.json"),
            json!({"schema":schema}).to_string(),
        )
        .unwrap();
        assert!(model::load(&path, None).err().unwrap().contains(schema));
    }
    let duplicate=br#"{"schema":"nrl.model-definition/0","descriptor":{"state":"stateless","state":"recurrent"}}"#;
    assert!(Definition::parse(duplicate)
        .err()
        .unwrap()
        .contains("duplicate"));
    assert!(Definition::parse(&vec![b' '; model::MAX_MODEL_BYTES + 1]).is_err());
    fs::write(path.join("manifest.json"), &raw).unwrap();
    let loaded = model::load(&path, None).unwrap();
    let patch = root.join("patch");
    loaded
        .overlay(replacements())
        .unwrap()
        .save(&patch)
        .unwrap();
    let mut bad: Value =
        serde_json::from_slice(&fs::read(patch.join("manifest.json")).unwrap()).unwrap();
    bad["sources"][0]["licence"] = json!("discarded source notice");
    write_manifest(&patch, &bad);
    assert!(model::load(&patch, Some(&loaded)).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replacements_and_output_files_refuse_overwrites_and_boundary_changes() {
    let model = base();
    let mut unknown = replacements();
    unknown
        .objects
        .insert("unknown".into(), unknown.objects["head"].clone());
    assert!(model.overlay(unknown).is_err());
    let mut wrong = replacements();
    if let model::ObjectData::Affine(a) = wrong.objects.get_mut("head").unwrap() {
        a.shape = [3, 1];
        a.weights.truncate(3);
        a.bias.truncate(1);
    }
    assert!(model.overlay(wrong).is_err());
    let mut unchanged = replacements();
    unchanged.objects.clear();
    unchanged
        .objects
        .insert("head".into(), model.object("head").unwrap().clone());
    assert!(model.overlay(unchanged).is_err());
    let root = directory();
    let path = root.join("saved");
    model.save(&path).unwrap();
    let before = fs::read(path.join("manifest.json")).unwrap();
    assert!(model.save(&path).is_err());
    assert_eq!(fs::read(path.join("manifest.json")).unwrap(), before);
    let output = root.join("prediction.json");
    let input = vector(&oracle()["input"]);
    model.run_to_file(&input, &output).unwrap();
    let before = fs::read(&output).unwrap();
    assert!(model.run_to_file(&input, &output).is_err());
    assert_eq!(fs::read(output).unwrap(), before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_export_cli_reports_the_written_package_identity() {
    let root = directory();
    let base_dir = root.join("base");
    let overlay_dir = root.join("overlay");
    let flat_dir = root.join("flat");
    let source = base();
    source.save(&base_dir).unwrap();
    source
        .overlay(replacements())
        .unwrap()
        .save(&overlay_dir)
        .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_nrl-lab"))
        .arg("export-standalone")
        .arg(&overlay_dir)
        .arg(&flat_dir)
        .arg(&base_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reported: Value = serde_json::from_slice(&output.stdout).unwrap();
    let saved = model::load(&flat_dir, None).unwrap();
    assert_eq!(reported, saved.info().unwrap());
    assert_eq!(reported["kind"], "standalone");
    assert_eq!(reported["dependency_bytes"], 0);
    fs::remove_dir_all(root).unwrap();
}
