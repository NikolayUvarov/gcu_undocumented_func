use nrl_core::{Error, Shape};
use nrl_lab::{close, evaluate, svd, Artifact};
use nrl_repr::factor::{Factor, FactorGradients, SvdBlock};
use serde_json::Value;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/f64-v4.json")).unwrap()
}
fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn jacobi_singular_values_reconstruction_and_truncation_match_external_f64_oracle() {
    for case in fixtures()["svd_initialization"].as_array().unwrap() {
        let inputs = case["inputs"].as_u64().unwrap() as usize;
        let outputs = case["outputs"].as_u64().unwrap() as usize;
        let weights = vector(&case["weights"]);
        let singular = vector(&case["singular_values"]);
        let scale = weights
            .iter()
            .fold(0.0_f64, |a, b| a.max(b.abs()))
            .max(1e-300);
        for rank in 1..=inputs.min(outputs) {
            let result = svd::decompose(inputs, outputs, rank, &weights).unwrap();
            assert!((1..=svd::MAX_SWEEPS).contains(&result.sweeps));
            close(
                &result
                    .singular_values
                    .iter()
                    .map(|s| s / scale)
                    .collect::<Vec<_>>(),
                &singular.iter().map(|s| s / scale).collect::<Vec<_>>(),
            )
            .unwrap();
            let factor =
                Factor::new(inputs, outputs, rank, &result.left, &result.right, &[]).unwrap();
            let mut reconstructed = vec![0.0; weights.len()];
            factor.reconstruct(&mut reconstructed).unwrap();
            let residual: f64 = reconstructed
                .iter()
                .zip(&weights)
                .map(|(a, b)| ((a - b) / scale).powi(2))
                .sum();
            let expected: f64 = singular[rank..].iter().map(|s| (s / scale).powi(2)).sum();
            assert!(
                (residual - expected).abs() < 1e-10 * (1.0 + expected),
                "rank {rank}: {residual} vs {expected}"
            );
            if rank == inputs.min(outputs) {
                close(
                    &reconstructed.iter().map(|w| w / scale).collect::<Vec<_>>(),
                    &weights.iter().map(|w| w / scale).collect::<Vec<_>>(),
                )
                .unwrap();
            }
            let input: Vec<_> = (0..inputs).map(|i| (i as f64 + 1.0) * 0.1).collect();
            let mut output = vec![0.0; outputs];
            factor
                .forward(&input, &mut output, &mut vec![0.0; rank])
                .unwrap();
            let expected: Vec<f64> = reconstructed
                .chunks_exact(inputs)
                .map(|row| row.iter().zip(&input).map(|(a, b)| a * b).sum())
                .collect();
            close(&output, &expected).unwrap();
        }
    }
}

#[test]
fn full_rank_control_matches_dense_and_executes_without_materialized_projection_arrays() {
    let fixture = fixtures();
    let source: Artifact =
        serde_json::from_value(fixture["channel_selection"]["source"].clone()).unwrap();
    let input = vector(&fixture["cases"][0]["input"]);
    let control = svd::control(&source, 3).unwrap();
    assert!(control.gate.is_empty() && control.up.is_empty() && control.down.is_empty());
    assert!(control.spline.is_none() && control.selection.is_none());
    close(
        &control.forward(&input).unwrap(),
        &source.forward(&input).unwrap(),
    )
    .unwrap();
    assert_eq!(
        control.block().unwrap().scratch_len(),
        7 * source.shape[1] + 3
    );
    let pruned = nrl_lab::prune(&source, 2, None).unwrap();
    assert!(svd::control(&pruned, 2).is_err());
}

#[test]
fn factor_backward_accumulates_and_explicit_buffer_shapes_are_checked() {
    let factor = Factor::new(
        2,
        3,
        2,
        &[1.0, 2.0, -1.0, 0.5, 0.0, 3.0],
        &[0.2, -0.1, 0.5, 0.4],
        &[],
    )
    .unwrap();
    let x = [0.3, -0.7];
    let dy = [0.2, -0.4, 0.8];
    let mut dx = [0.0; 2];
    let mut left = [0.0; 6];
    let mut right = [0.0; 4];
    factor
        .backward(
            &x,
            &dy,
            &mut dx,
            &mut FactorGradients {
                bias: &mut [],
                left: &mut left,
                right: &mut right,
            },
            &mut [0.0; 2],
        )
        .unwrap();
    let once = (dx, left, right);
    factor
        .backward(
            &x,
            &dy,
            &mut dx,
            &mut FactorGradients {
                bias: &mut [],
                left: &mut left,
                right: &mut right,
            },
            &mut [0.0; 2],
        )
        .unwrap();
    close(&dx, &once.0.map(|v| 2.0 * v)).unwrap();
    close(&left, &once.1.map(|v| 2.0 * v)).unwrap();
    close(&right, &once.2.map(|v| 2.0 * v)).unwrap();
    assert_eq!(
        factor.forward(&x, &mut [0.0; 3], &mut []),
        Err(Error::Shape)
    );
    assert_eq!(
        factor.backward(
            &x,
            &dy,
            &mut dx,
            &mut FactorGradients {
                bias: &mut [],
                left: &mut [],
                right: &mut right
            },
            &mut [0.0; 2]
        ),
        Err(Error::Shape)
    );
    assert!(Factor::new(2, 3, 0, &[], &[], &[]).is_err());
    assert!(Factor::new(2, 3, 3, &[0.0; 9], &[0.0; 6], &[]).is_err());
    assert!(Factor::new(1, 1, 1, &[f64::NAN], &[1.0], &[]).is_err());
    assert!(Factor::new(1, 1, 1, &[f64::MAX], &[2.0], &[])
        .unwrap()
        .reconstruct(&mut [0.0])
        .is_err());
    assert!(SvdBlock::new(Shape::new(2, 3, 2).unwrap(), factor, factor, factor).is_err());
}

#[test]
fn initializer_refuses_invalid_limits_nonfinite_data_and_nonconvergence() {
    for (n, m, rank, weights) in [
        (0, 1, 1, vec![]),
        (2, 2, 0, vec![0.0; 4]),
        (2, 2, 3, vec![0.0; 4]),
        (65, 1, 1, vec![0.0; 65]),
        (2, 2, 1, vec![0.0; 3]),
        (1, 1, 1, vec![f64::INFINITY]),
    ] {
        assert!(svd::decompose(n, m, rank, &weights).is_err());
    }
    assert!(svd::with_sweep_limit(2, 2, 1, &[1.0; 4], 0).is_err());
    assert!(svd::with_sweep_limit(2, 2, 1, &[1.0; 4], 129).is_err());
    let error = svd::with_sweep_limit(3, 3, 2, &[3.0, 1.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.0, 1.0], 1)
        .unwrap_err();
    assert!(error.contains("did not converge"));
}

#[test]
fn malformed_factor_artifacts_and_training_errors_leave_the_source_unchanged() {
    let fixture = fixtures();
    let artifact: Artifact =
        serde_json::from_value(fixture["cases"][4]["artifact"].clone()).unwrap();
    let input = vector(&fixture["cases"][4]["input"]);
    let target = vector(&fixture["cases"][4]["target"]);
    for rank in [0, 4, usize::MAX] {
        let mut bad = artifact.clone();
        bad.factors.as_mut().unwrap().rank = rank;
        assert!(bad.validate().is_err());
    }
    let mut bad = artifact.clone();
    bad.factors = None;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.gate.push(0.0);
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.factors.as_mut().unwrap().up_left.pop();
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.factors.as_mut().unwrap().down_right[0] = f64::NAN;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.selection = Some(nrl_lab::SelectionData {
        original_channels: 4,
        indices: vec![0, 1, 2, 3],
        method: "weight-energy-f64/1".into(),
    });
    assert!(bad.validate().is_err());
    let before = artifact.forward(&input).unwrap();
    let mut result = evaluate(&artifact, &input, &target).unwrap();
    result.gradients.factors.as_mut().unwrap().gate_right.pop();
    assert!(artifact.step(&result.gradients, 0.05).is_err());
    assert_eq!(artifact.forward(&input).unwrap(), before);
    let mut biased = fixture["cases"][4]["artifact"].clone();
    biased["bias"] = serde_json::json!([0.1]);
    assert!(serde_json::from_value::<Artifact>(biased).is_err());
}
