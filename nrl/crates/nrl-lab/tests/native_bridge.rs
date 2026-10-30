use nrl_lab::{
    model::{self, AffineData, Definition, Model, Node, ObjectData, Operation, Replacements, Tie},
    native_bridge, Artifact, BiasData,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nrl-portable-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn definition() -> Definition {
    Definition::parse(include_bytes!(
        "../../../tests/fixtures/model-v0/definition.json"
    ))
    .unwrap()
}
fn plain() -> Definition {
    Definition::parse(include_bytes!(
        "../../../tests/fixtures/model-v0/plain.json"
    ))
    .unwrap()
}
fn values(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| ((i * 37 % 29) as f64 - 14.0) * 0.01)
        .collect()
}
fn verify(temp: &Temp, name: &str, model: Model) -> Value {
    let directory = temp.0.join(name);
    model.save(&directory).unwrap();
    let input = values(model.manifest().descriptor.configuration.input_width * 3);
    let mut report =
        native_bridge::check(&directory, &input).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(report["model_id"], model.model_id());
    assert_eq!(report["native_execution"], false);
    assert_eq!(report["service_state_released"], true);
    report["case"] = name.into();
    report.as_object_mut().unwrap().remove("output");
    println!("BRIDGE_CASE {}", serde_json::to_string(&report).unwrap());
    report
}
fn single(object: ObjectData) -> Definition {
    let summary = object.summary().unwrap();
    let op = match &object {
        ObjectData::Affine(_) => Operation::Affine {
            input: "input".into(),
            object: "weights".into(),
        },
        ObjectData::Gated(_) => Operation::Gated {
            input: "input".into(),
            object: "weights".into(),
        },
    };
    let mut d = plain();
    d.objects = BTreeMap::from([("weights".into(), object)]);
    let g = &mut d.descriptor.configuration;
    g.input_width = summary.input_width;
    g.nodes = vec![Node {
        id: "prediction".into(),
        op,
    }];
    g.output = "prediction".into();
    d.descriptor.tied_objects.clear();
    d
}
fn dense(inputs: usize, channels: usize, outputs: usize, bias: bool) -> Artifact {
    let ObjectData::Gated(mut a) = definition().objects.remove("block0").unwrap() else {
        panic!("not gated")
    };
    a.shape = [inputs, channels, outputs];
    a.gate = values(inputs * channels);
    a.up = values(inputs * channels);
    a.down = values(channels * outputs);
    a.bias = bias.then(|| BiasData {
        gate: values(channels),
        up: values(channels),
        down: values(outputs),
    });
    a.validate().unwrap();
    a
}

#[test]
fn fresh_transformed_models_cover_shapes_biases_splines_pruning_and_ranks() {
    let temp = Temp::new();
    let mut count = 0;
    for (inputs, channels, outputs) in [(1, 1, 1), (2, 3, 2), (3, 4, 2), (5, 6, 4)] {
        for bias in [false, true] {
            let source = dense(inputs, channels, outputs, bias);
            let pruned = nrl_lab::prune(&source, channels.saturating_sub(1).max(1), None).unwrap();
            let width = pruned.shape[1];
            let mut candidates = vec![
                ("dense".to_owned(), source.clone()),
                ("pruned".into(), pruned.clone()),
            ];
            for knots in [2, 5] {
                candidates.push((
                    format!("linear-{knots}"),
                    nrl_lab::linear_spline_control(
                        &pruned,
                        knots,
                        vec![-1.0; width],
                        vec![1.0; width],
                    )
                    .unwrap(),
                ));
            }
            for knots in [3, 6] {
                candidates.push((
                    format!("cubic-{knots}"),
                    nrl_lab::cubic_spline_control(
                        &pruned,
                        knots,
                        vec![-1.0; width],
                        vec![1.0; width],
                    )
                    .unwrap(),
                ));
            }
            for rank in 1..=inputs.min(channels).min(outputs).min(2) {
                candidates.push((
                    format!("svd-{rank}"),
                    nrl_lab::svd::control(&source, rank).unwrap(),
                ));
            }
            for (kind, artifact) in candidates {
                let name = format!("shape-{inputs}-{channels}-{outputs}-bias-{bias}-{kind}");
                verify(
                    &temp,
                    &name,
                    Model::from_definition(single(ObjectData::Gated(artifact))).unwrap(),
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 62);
}

#[test]
fn graph_lineage_metadata_and_parameter_boundaries_match_the_exporter() {
    let temp = Temp::new();
    let mut identity_output = definition();
    identity_output.descriptor.configuration.output = "input".into();
    assert!(Model::from_definition(identity_output).is_err());
    for output in ["left", "features", "g0"] {
        let mut d = definition();
        d.descriptor.configuration.output = output.into();
        let mut needed = BTreeSet::from([output.to_owned()]);
        for node in d.descriptor.configuration.nodes.iter().rev() {
            if !needed.contains(&node.id) {
                continue;
            }
            match &node.op {
                Operation::Affine { input, .. }
                | Operation::Gated { input, .. }
                | Operation::Silu { input }
                | Operation::Relu { input } => {
                    needed.insert(input.clone());
                }
                Operation::Add { left, right } => {
                    needed.insert(left.clone());
                    needed.insert(right.clone());
                }
            }
        }
        d.descriptor
            .configuration
            .nodes
            .retain(|node| needed.contains(&node.id));
        let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for node in &d.descriptor.configuration.nodes {
            if let Operation::Affine { object, .. } | Operation::Gated { object, .. } = &node.op {
                uses.entry(object.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }
        d.objects.retain(|key, _| uses.contains_key(key));
        d.descriptor.tied_objects = uses
            .into_iter()
            .filter(|(_, nodes)| nodes.len() > 1)
            .map(|(object, nodes)| Tie { object, nodes })
            .collect();
        verify(
            &temp,
            &format!("output-{output}"),
            Model::from_definition(d).unwrap(),
        );
    }
    let mut d = definition();
    d.descriptor.tasks = vec!["classification-logits/0".into()];
    d.sources[0].identifier = "NRL: Кириллица ✓ 𝄞 \" \\".into();
    d.sources[0].notices = vec!["control \u{0}\u{8}\u{c}\n\t and Unicode é".into()];
    verify(
        &temp,
        "unicode-metadata",
        Model::from_definition(d).unwrap(),
    );
    let base = Model::from_definition(definition()).unwrap();
    let replacements: Replacements = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/model-v0/replacements.json"
    ))
    .unwrap();
    let flat = base.overlay(replacements).unwrap().standalone().unwrap();
    assert!(!flat.manifest().history.is_empty());
    verify(&temp, "materialized-history", flat);
    for (inputs, outputs, bias) in [(127, 128, true), (128, 128, false)] {
        let a = AffineData {
            schema: "nrl.affine/0".into(),
            shape: [inputs, outputs],
            weights: values(inputs * outputs),
            bias: if bias { values(outputs) } else { vec![] },
        };
        assert_eq!(a.weights.len() + a.bias.len(), 16384);
        verify(
            &temp,
            &format!("value-bound-bias-{bias}"),
            Model::from_definition(single(ObjectData::Affine(a))).unwrap(),
        );
    }
    let mut d = single(ObjectData::Affine(AffineData {
        schema: "nrl.affine/0".into(),
        shape: [1, 1],
        weights: vec![0.9],
        bias: vec![0.001],
    }));
    let object = d.objects["weights"].clone();
    d.objects.clear();
    d.descriptor.configuration.nodes.clear();
    let mut previous = "input".to_owned();
    for i in 0..64 {
        let name = format!("node{i:02}");
        let key = format!("object{i:02}");
        d.objects.insert(key.clone(), object.clone());
        d.descriptor.configuration.nodes.push(Node {
            id: name.clone(),
            op: Operation::Affine {
                input: previous,
                object: key,
            },
        });
        previous = name;
    }
    d.descriptor.configuration.output = previous;
    assert_eq!(d.objects.len(), 64);
    let r = verify(
        &temp,
        "node-and-object-bound",
        Model::from_definition(d).unwrap(),
    );
    assert_eq!(r["workspace_values"], 65);
}

#[test]
fn profile_refusals_and_report_publication_remain_explicit() {
    let temp = Temp::new();
    let directory = temp.0.join("model");
    Model::from_definition(plain())
        .unwrap()
        .save(&directory)
        .unwrap();
    let input = temp.0.join("input.json");
    let report = temp.0.join("report.json");
    fs::write(&input, serde_json::to_vec(&values(6)).unwrap()).unwrap();
    native_bridge::check_to_file(&directory, &input, &report).unwrap();
    let saved = fs::read(&report).unwrap();
    assert!(native_bridge::check_to_file(&directory, &input, &report).is_err());
    assert_eq!(fs::read(&report).unwrap(), saved);
    for input in [vec![], vec![0.1], vec![f64::NAN; 2], vec![0.1; 514]] {
        assert!(native_bridge::check(&directory, &input).is_err());
    }
    let mut d = single(ObjectData::Affine(AffineData {
        schema: "nrl.affine/0".into(),
        shape: [1, 128],
        weights: values(128),
        bias: values(128),
    }));
    d.execution.max_batch_rows = 128;
    let wide = temp.0.join("wide");
    Model::from_definition(d).unwrap().save(&wide).unwrap();
    let error = native_bridge::check(&wide, &vec![0.1; 65]).unwrap_err();
    assert!(error.contains("payload limit"), "{error}");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    let record = manifest["objects"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    let path = directory.join(record["file"].as_str().unwrap());
    let object: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let compact = serde_json::to_vec(&object).unwrap();
    record["bytes"] = compact.len().into();
    record["sha256"] = nrl_lab::qsl::hash(&compact).into();
    fs::write(path, compact).unwrap();
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(model::load(&directory, None).is_ok());
    let error = native_bridge::check(&directory, &values(6)).unwrap_err();
    assert!(
        error.contains("WRITER") || error.contains("PROFILE"),
        "{error}"
    );
    assert!(report.exists());
}
