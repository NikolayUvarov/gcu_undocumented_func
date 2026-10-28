use nrl_lab::{close, cubic_spline_control, prune, Artifact};
use nrl_repr::Spline;
use serde_json::Value;

fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn hermite_matches_reference_at_knots_interior_and_linear_tails() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/f64-v5.json")).unwrap();
    let f = &fixture["cubic"];
    let low = vector(&f["low"]);
    let high = vector(&f["high"]);
    let values = vector(&f["values"]);
    let spline = Spline::hermite(2, 4, &low, &high, &values).unwrap();
    let mut output = Vec::new();
    let mut dx = Vec::new();
    let mut dv = vec![0.0; values.len()];
    for (i, x) in vector(&f["input"]).iter().enumerate() {
        let s = spline.sample(i % 2, *x).unwrap();
        output.push(s.value);
        dx.push(s.derivative);
        for k in 0..4 {
            dv[s.indices[k]] += s.weights[k];
        }
    }
    close(&output, &vector(&f["output"])).unwrap();
    close(&dx, &vector(&f["input_gradient"])).unwrap();
    close(&dv, &vector(&f["values_gradient"])).unwrap();
}

#[test]
fn two_knots_reduce_to_linear_and_cubic_tails_never_grow_cubically() {
    let low = [-2.0];
    let high = [4.0];
    let values = [1.0, 7.0];
    let cubic = Spline::hermite(1, 2, &low, &high, &values).unwrap();
    let linear = Spline::new(1, 2, &low, &high, &values).unwrap();
    for x in [-1e30, -3.0, -2.0, -0.7, 0.0, 3.5, 4.0, 5.0, 1e30] {
        let a = cubic.sample(0, x).unwrap();
        let b = linear.sample(0, x).unwrap();
        close(&[a.value, a.derivative], &[b.value, b.derivative]).unwrap();
    }
    assert!(Spline::hermite(1, 1, &low, &high, &[1.0]).is_err());
    assert!(cubic.sample(0, f64::NAN).is_err());
}

#[test]
fn cubic_is_c1_at_every_knot_and_coefficient_derivatives_match_finite_differences() {
    let low = [-2.0];
    let high = [4.0];
    let values = [0.2, -0.4, 1.7, 0.8];
    let spline = Spline::hermite(1, 4, &low, &high, &values).unwrap();
    for k in 0..4 {
        let x = -2.0 + k as f64 * 2.0;
        let at = spline.sample(0, x).unwrap();
        assert_eq!(at.value, values[k]);
        for delta in [-1e-8, 1e-8] {
            let near = spline.sample(0, x + delta).unwrap();
            assert!((near.value - at.value).abs() < 1e-7);
            assert!((near.derivative - at.derivative).abs() < 1e-7);
        }
    }
    for x in [-3.0, -2.0, -1.1, 0.0, 0.8, 2.0, 3.1, 4.0, 5.0] {
        let sample = spline.sample(0, x).unwrap();
        let mut analytical = [0.0; 4];
        for i in 0..4 {
            analytical[sample.indices[i]] += sample.weights[i];
        }
        for parameter in 0..4 {
            let mut plus = values;
            let mut minus = values;
            plus[parameter] += 1e-6;
            minus[parameter] -= 1e-6;
            let p = Spline::hermite(1, 4, &low, &high, &plus)
                .unwrap()
                .sample(0, x)
                .unwrap()
                .value;
            let m = Spline::hermite(1, 4, &low, &high, &minus)
                .unwrap()
                .sample(0, x)
                .unwrap()
                .value;
            assert!(((p - m) / 2e-6 - analytical[parameter]).abs() < 1e-8);
        }
        let p = spline.sample(0, x + 1e-6).unwrap().value;
        let m = spline.sample(0, x - 1e-6).unwrap().value;
        assert!(((p - m) / 2e-6 - sample.derivative).abs() < 2e-6);
    }
}

#[test]
fn cubic_control_preserves_the_shared_channel_mapping_and_projections() {
    let f: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/f64-v5.json")).unwrap();
    let source: Artifact =
        serde_json::from_value(f["channel_selection"]["source"].clone()).unwrap();
    let pruned = prune(&source, 2, Some(&vector(&f["channel_selection"]["input"]))).unwrap();
    let cubic = cubic_spline_control(&pruned, 4, vec![-2.0; 2], vec![2.0; 2]).unwrap();
    assert_eq!(cubic.representation, "spline-cubic");
    assert_eq!(cubic.gate, pruned.gate);
    assert_eq!(cubic.up, pruned.up);
    assert_eq!(cubic.down, pruned.down);
    assert_eq!(
        cubic.selection.unwrap().indices,
        pruned.selection.unwrap().indices
    );
}
