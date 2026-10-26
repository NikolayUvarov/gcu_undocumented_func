use nrl_core::{Error, Shape};
use nrl_lab::{close, linear_spline_control, prune, Artifact};
use nrl_repr::{pruning, Activation, Block};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/f64-v3.json")).unwrap()
}
fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn ranking_and_packed_weights_match_external_reference_and_full_width_is_identity() {
    let f = fixture();
    let f = &f["channel_selection"];
    let source: Artifact = serde_json::from_value(f["source"].clone()).unwrap();
    let x = vector(&f["input"]);
    for (calibration, name, scores_name) in [
        (None, "weight_indices", "weight_scores_f64"),
        (
            Some(x.as_slice()),
            "calibration_indices",
            "calibration_scores_f64",
        ),
    ] {
        let mut scores = vec![0.0; 4];
        pruning::scores(
            &source.compact_block().unwrap(),
            calibration,
            &mut scores,
            &mut [0.0; 12],
        )
        .unwrap();
        close(&scores, &vector(&f[scores_name])).unwrap();
        let result = prune(&source, 2, calibration).unwrap();
        let expected: Vec<usize> = serde_json::from_value(f[name].clone()).unwrap();
        assert_eq!(result.selection.as_ref().unwrap().indices, expected);
        for (dest, c) in expected.iter().enumerate() {
            assert_eq!(
                result.gate[dest * 3..dest * 3 + 3],
                source.gate[c * 3..c * 3 + 3]
            );
            assert_eq!(
                result.up[dest * 3..dest * 3 + 3],
                source.up[c * 3..c * 3 + 3]
            );
            for o in 0..3 {
                assert_eq!(result.down[o * 2 + dest], source.down[o * 4 + c]);
            }
        }
        let full = prune(&source, 4, calibration).unwrap();
        assert_eq!(full.forward(&x).unwrap(), source.forward(&x).unwrap());
    }
    assert_ne!(f["weight_indices"], f["calibration_indices"]);
}

#[test]
fn spline_control_uses_identical_channels_and_preserves_trainable_projection_weights() {
    let f = fixture();
    let f = &f["channel_selection"];
    let source: Artifact = serde_json::from_value(f["source"].clone()).unwrap();
    let pruned = prune(&source, 2, Some(&vector(&f["input"]))).unwrap();
    let spline = linear_spline_control(&pruned, 4, vec![-2.0; 2], vec![2.0; 2]).unwrap();
    assert_eq!(spline.gate, pruned.gate);
    assert_eq!(spline.up, pruned.up);
    assert_eq!(spline.down, pruned.down);
    assert_eq!(
        spline.selection.unwrap().indices,
        pruned.selection.unwrap().indices
    );
    assert!(linear_spline_control(&source, 4, vec![-2.0; 4], vec![2.0; 4]).is_err());
}

#[test]
fn heap_selection_agrees_with_an_independent_full_sort_including_ties() {
    for len in 1..24 {
        let scores: Vec<_> = (0..len).map(|i| ((i * 7 + len) % 5) as f64).collect();
        for width in 1..=len {
            let mut expected: Vec<_> = (0..len).collect();
            expected.sort_by(|a, b| scores[*b].total_cmp(&scores[*a]).then(a.cmp(b)));
            expected.truncate(width);
            expected.sort_unstable();
            let mut actual = vec![usize::MAX; width];
            pruning::select(&scores, &mut actual).unwrap();
            assert_eq!(actual, expected);
        }
    }
    let mut indices = [usize::MAX; 2];
    pruning::select(&[0.0; 4], &mut indices).unwrap();
    assert_eq!(indices, [0, 1]);
}

#[test]
fn calibration_caps_rows_explicitly_and_refuses_empty_or_malformed_inputs() {
    let f = fixture();
    let source: Artifact =
        serde_json::from_value(f["channel_selection"]["source"].clone()).unwrap();
    let mut x = vec![0.0; pruning::MAX_CALIBRATION_ROWS * 3];
    for row in x.chunks_exact_mut(3) {
        row[0] = 1.0;
    }
    let before = prune(&source, 2, Some(&x)).unwrap();
    x.extend([0.0, 0.0, 1e12]);
    let after = prune(&source, 2, Some(&x)).unwrap();
    assert_eq!(
        before.selection.unwrap().indices,
        after.selection.unwrap().indices
    );
    for input in [&[][..], &[1.0][..], &[f64::NAN; 3][..]] {
        assert!(prune(&source, 2, Some(input)).is_err());
    }
    assert!(prune(&source, 0, None).is_err());
    assert!(prune(&source, 5, None).is_err());
}

#[test]
fn nonfinite_scores_overflow_and_invalid_mappings_are_refused() {
    for scores in [&[f64::NAN][..], &[f64::INFINITY][..], &[-1.0][..]] {
        assert!(pruning::select(scores, &mut [0]).is_err());
    }
    assert!(pruning::select(&[1.0], &mut []).is_err());
    assert!(pruning::select(&[1.0], &mut [0, 0]).is_err());
    let shape = Shape::new(1, 1, 1).unwrap();
    let block = Block::new(shape, &[f64::MAX], &[1.0], &[0.0], Activation::Silu).unwrap();
    assert_eq!(
        pruning::scores(&block, None, &mut [0.0], &mut [0.0; 3]),
        Err(Error::NonFinite)
    );
    let f = fixture();
    let artifact: Artifact = serde_json::from_value(f["cases"][2]["artifact"].clone()).unwrap();
    for indices in [vec![0, 0], vec![3, 0], vec![0, 4], vec![]] {
        let mut bad = artifact.clone();
        bad.selection.as_mut().unwrap().indices = indices;
        assert!(bad.validate().is_err());
    }
    let mut bad = artifact.clone();
    bad.selection = None;
    assert!(bad.validate().is_err());
    let mut bad = artifact.clone();
    bad.selection.as_mut().unwrap().method = "unknown".into();
    assert!(bad.validate().is_err());
    let mut bad = artifact;
    bad.schema = "nrl.block/0".into();
    assert!(bad.validate().unwrap_err().contains("nrl.block/0"));
}
