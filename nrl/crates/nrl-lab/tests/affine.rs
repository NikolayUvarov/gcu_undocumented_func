use nrl_lab::{
    close, cubic_spline_control, evaluate, linear_spline_control, prune, svd, Artifact, BiasData,
};
use nrl_repr::bias::{BiasGradients, Biases};
use nrl_repr::{Activation, Block, Gradients};
use serde_json::Value;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/f64-v5.json")).unwrap()
}
fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
fn source() -> Artifact {
    serde_json::from_value(fixtures()["affine_channel_selection"]["source"].clone()).unwrap()
}

#[test]
fn affine_channel_scores_and_selected_biases_match_independent_torch_oracle() {
    let f = fixtures();
    let f = &f["affine_channel_selection"];
    let source = source();
    let input = vector(&f["input"]);
    for (calibration, indices_name, scores_name) in [
        (None, "weight_indices", "weight_scores_f64"),
        (
            Some(input.as_slice()),
            "calibration_indices",
            "calibration_scores_f64",
        ),
    ] {
        let block = source.compact_block().unwrap();
        let mut scores = [0.0; 4];
        nrl_repr::pruning::scores(&block, calibration, &mut scores, &mut [0.0; 12]).unwrap();
        close(&scores, &vector(&f[scores_name])).unwrap();
        let pruned = prune(&source, 2, calibration).unwrap();
        let expected: Vec<usize> = serde_json::from_value(f[indices_name].clone()).unwrap();
        assert_eq!(pruned.selection.as_ref().unwrap().indices, expected);
        let b = source.bias.as_ref().unwrap();
        let p = pruned.bias.as_ref().unwrap();
        assert_eq!(
            p.gate,
            expected.iter().map(|i| b.gate[*i]).collect::<Vec<_>>()
        );
        assert_eq!(p.up, expected.iter().map(|i| b.up[*i]).collect::<Vec<_>>());
        assert_eq!(p.down, b.down);
        for spline in [
            linear_spline_control(&pruned, 4, vec![-2.0; 2], vec![2.0; 2]).unwrap(),
            cubic_spline_control(&pruned, 4, vec![-2.0; 2], vec![2.0; 2]).unwrap(),
        ] {
            let s = spline.bias.unwrap();
            assert_eq!(s.gate, p.gate);
            assert_eq!(s.up, p.up);
            assert_eq!(s.down, p.down);
        }
        let identity = prune(&source, 4, calibration).unwrap();
        assert_eq!(
            identity.forward(&input).unwrap(),
            source.forward(&input).unwrap()
        );
    }
}

#[test]
fn full_rank_svd_preserves_every_output_bias_and_affine_outputs() {
    let source = source();
    let input = vector(&fixtures()["cases"][0]["input"]);
    let factored = svd::control(&source, 3).unwrap();
    for (a, b) in factored
        .bias
        .as_ref()
        .unwrap()
        .tensors()
        .iter()
        .zip(source.bias.as_ref().unwrap().tensors())
    {
        assert_eq!(*a, b);
    }
    close(
        &factored.forward(&input).unwrap(),
        &source.forward(&input).unwrap(),
    )
    .unwrap();
    let mut zero_down = factored.clone();
    zero_down.factors.as_mut().unwrap().down_left.fill(0.0);
    let expected: Vec<_> = (0..input.len() / 3)
        .flat_map(|_| source.bias.as_ref().unwrap().down.iter().copied())
        .collect();
    assert_eq!(zero_down.forward(&input).unwrap(), expected);
}

#[test]
fn absent_zero_and_mixed_biases_have_explicit_semantics() {
    let fixture = fixtures();
    for i in 0..5 {
        let original: Artifact =
            serde_json::from_value(fixture["cases"][i]["artifact"].clone()).unwrap();
        let input = vector(&fixture["cases"][i]["input"]);
        let mut zero = original.clone();
        zero.bias = Some(BiasData {
            gate: vec![0.0; original.shape[1]],
            up: vec![0.0; original.shape[1]],
            down: vec![0.0; original.shape[2]],
        });
        assert_eq!(
            original.forward(&input).unwrap(),
            zero.forward(&input).unwrap()
        );
        let mut mixed = zero.clone();
        mixed.bias.as_mut().unwrap().up.clear();
        mixed.bias.as_mut().unwrap().down.clear();
        assert_eq!(
            original.forward(&input).unwrap(),
            mixed.forward(&input).unwrap()
        );
        let result = evaluate(
            &mixed,
            &input,
            &vec![0.1; input.len() / original.shape[0] * original.shape[2]],
        )
        .unwrap();
        assert_eq!(
            result.gradients.bias.as_ref().unwrap().gate.len(),
            original.shape[1]
        );
        assert!(result.gradients.bias.as_ref().unwrap().up.is_empty());
        assert!(mixed.step(&result.gradients, 1e-6).is_ok());
    }
}

#[test]
fn bias_gradients_accumulate_while_compact_input_gradient_is_replaced() {
    let shape = nrl_core::Shape::new(1, 1, 1).unwrap();
    let block = Block::new(
        shape,
        &[0.2],
        &[0.3],
        &[0.4],
        Activation::Silu,
        Biases {
            gate: &[0.1],
            up: &[-0.2],
            down: &[0.3],
        },
    )
    .unwrap();
    let mut bg = [0.0];
    let mut bu = [0.0];
    let mut bd = [0.0];
    let mut gate = [0.0];
    let mut up = [0.0];
    let mut down = [0.0];
    let mut dx = [99.0];
    block
        .backward(
            &[0.7],
            &[0.8],
            &mut dx,
            &mut Gradients {
                gate: &mut gate,
                up: &mut up,
                down: &mut down,
                values: &mut [],
                biases: BiasGradients {
                    gate: &mut bg,
                    up: &mut bu,
                    down: &mut bd,
                },
            },
            &mut [0.0; 3],
        )
        .unwrap();
    let first = (dx[0], bg[0], bu[0], bd[0]);
    block
        .backward(
            &[0.7],
            &[0.8],
            &mut dx,
            &mut Gradients {
                gate: &mut gate,
                up: &mut up,
                down: &mut down,
                values: &mut [],
                biases: BiasGradients {
                    gate: &mut bg,
                    up: &mut bu,
                    down: &mut bd,
                },
            },
            &mut [0.0; 3],
        )
        .unwrap();
    assert_eq!(dx[0], first.0);
    close(
        &[bg[0], bu[0], bd[0]],
        &[2.0 * first.1, 2.0 * first.2, 2.0 * first.3],
    )
    .unwrap();
}

#[test]
fn descriptor_refuses_unsupported_contracts_and_devices_without_fallback() {
    let f = fixtures();
    for case in f["cases"].as_array().unwrap() {
        let original: Artifact = serde_json::from_value(case["artifact"].clone()).unwrap();
        assert!(original.block_on("cpu-scalar").is_ok());
        assert!(original
            .block_on("cuda:0")
            .err()
            .unwrap()
            .contains("unsupported device"));
        for (field, value) in [
            ("kind", "affine/0"),
            ("source_activation", "relu"),
            ("dtype", "f32"),
            ("layout", "column-major"),
            ("state", "kv-cache"),
            ("parameter_sharing", "tied"),
        ] {
            let mut bad = case["artifact"].clone();
            bad["descriptor"][field] = Value::String(value.into());
            let bad: Artifact = serde_json::from_value(bad).unwrap();
            assert!(bad.validate().unwrap_err().contains(value));
            assert!(prune(&bad, 1, None).is_err());
            assert!(svd::control(&bad, 1).is_err());
        }
    }
    let mut missing = f["cases"][0]["artifact"].clone();
    missing.as_object_mut().unwrap().remove("descriptor");
    assert!(serde_json::from_value::<Artifact>(missing).is_err());
    let mut unknown = f["cases"][0]["artifact"].clone();
    unknown["descriptor"]["unknown"] = Value::Bool(true);
    assert!(serde_json::from_value::<Artifact>(unknown).is_err());
}

#[test]
fn invalid_biases_and_gradient_mismatches_are_errors_without_source_mutation() {
    let source = source();
    let input = vector(&fixtures()["cases"][0]["input"]);
    for invalid in [vec![0.0; 3], vec![f64::NAN; 4], vec![f64::INFINITY; 4]] {
        let mut bad = source.clone();
        bad.bias.as_mut().unwrap().gate = invalid;
        assert!(bad.validate().is_err());
    }
    let mut large = source.clone();
    large.bias.as_mut().unwrap().down = vec![0.0; nrl_lab::MAX_ARTIFACT_VALUES + 1];
    assert!(large.validate().unwrap_err().contains("value limit"));
    let before = source.forward(&input).unwrap();
    let mut result = evaluate(&source, &input, &vec![0.1; before.len()]).unwrap();
    result.gradients.bias.as_mut().unwrap().gate.pop();
    assert!(source.step(&result.gradients, 0.01).is_err());
    assert_eq!(source.forward(&input).unwrap(), before);
    result.gradients.bias = None;
    assert!(source.step(&result.gradients, 0.01).is_err());
}
