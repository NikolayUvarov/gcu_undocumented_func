use crate::{Artifact, FactorData, Result};
use nrl_core::finite;

pub const MAX_SVD_DIMENSION: usize = 64;
pub const MAX_SWEEPS: usize = 128;

#[derive(Debug)]
pub struct Decomposition {
    pub left: Vec<f64>,
    pub right: Vec<f64>,
    pub singular_values: Vec<f64>,
    pub sweeps: usize,
}

// This bounded host initializer is not a large-model SVD backend.
pub fn decompose(
    inputs: usize,
    outputs: usize,
    rank: usize,
    weights: &[f64],
) -> Result<Decomposition> {
    with_sweep_limit(inputs, outputs, rank, weights, MAX_SWEEPS)
}

pub fn with_sweep_limit(
    inputs: usize,
    outputs: usize,
    rank: usize,
    weights: &[f64],
    sweep_limit: usize,
) -> Result<Decomposition> {
    if sweep_limit == 0 || sweep_limit > MAX_SWEEPS {
        return Err("SVD sweep limit must be in 1..=128".into());
    }
    if inputs == 0
        || outputs == 0
        || inputs > MAX_SVD_DIMENSION
        || outputs > MAX_SVD_DIMENSION
        || rank == 0
        || rank > inputs.min(outputs)
        || weights.len() != inputs * outputs
    {
        return Err("SVD expects a valid rank and a matrix with each dimension in 1..=64".into());
    }
    finite(weights).map_err(|e| format!("SVD input: {e:?}"))?;
    let transposed = inputs > outputs;
    let rows = inputs.max(outputs);
    let cols = inputs.min(outputs);
    let scale = weights.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
    let mut b = vec![0.0; rows * cols];
    if scale > 0.0 {
        for row in 0..rows {
            for col in 0..cols {
                b[row * cols + col] = if transposed {
                    weights[col * inputs + row]
                } else {
                    weights[row * inputs + col]
                } / scale;
            }
        }
    }
    let mut v = vec![0.0; cols * cols];
    for i in 0..cols {
        v[i * cols + i] = 1.0;
    }
    let mut converged = false;
    let mut sweeps = 0;
    for sweep in 0..sweep_limit {
        let mut changed = false;
        for p in 0..cols {
            for q in p + 1..cols {
                let mut a = 0.0;
                let mut d = 0.0;
                let mut cross = 0.0;
                for row in 0..rows {
                    let x = b[row * cols + p];
                    let y = b[row * cols + q];
                    a += x * x;
                    d += y * y;
                    cross += x * y;
                }
                if cross.abs() <= 1e-14 * a.sqrt() * d.sqrt() {
                    continue;
                }
                let tau = (d - a) / (2.0 * cross);
                if !tau.is_finite() {
                    return Err("SVD rotation exceeds the numerical range".into());
                }
                let t = if tau < 0.0 { -1.0 } else { 1.0 } / (tau.abs() + tau.hypot(1.0));
                let cosine = 1.0 / (1.0 + t * t).sqrt();
                let sine = t * cosine;
                for row in 0..rows {
                    let x = b[row * cols + p];
                    let y = b[row * cols + q];
                    b[row * cols + p] = cosine * x - sine * y;
                    b[row * cols + q] = sine * x + cosine * y;
                }
                for row in 0..cols {
                    let x = v[row * cols + p];
                    let y = v[row * cols + q];
                    v[row * cols + p] = cosine * x - sine * y;
                    v[row * cols + q] = sine * x + cosine * y;
                }
                changed = true;
            }
        }
        sweeps = sweep + 1;
        if !changed {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(format!("SVD did not converge within {sweep_limit} sweeps"));
    }
    let norms: Vec<f64> = (0..cols)
        .map(|c| {
            (0..rows)
                .map(|r| b[r * cols + c] * b[r * cols + c])
                .sum::<f64>()
                .sqrt()
        })
        .collect();
    let mut order: Vec<usize> = (0..cols).collect();
    order.sort_by(|a, b| norms[*b].total_cmp(&norms[*a]).then(a.cmp(b)));
    let singular_values: Vec<f64> = order.iter().map(|c| norms[*c] * scale).collect();
    finite(&singular_values).map_err(|_| "SVD singular value overflow")?;
    let mut tall_left = vec![0.0; rows * rank];
    let mut tall_right = vec![0.0; rank * cols];
    for r in 0..rank {
        let c = order[r];
        let root = singular_values[r].sqrt();
        if norms[c] > 0.0 {
            for row in 0..rows {
                tall_left[row * rank + r] = b[row * cols + c] / norms[c] * root;
            }
            for col in 0..cols {
                tall_right[r * cols + col] = root * v[col * cols + c];
            }
        }
    }
    let (left, right) = if transposed {
        let mut left = vec![0.0; outputs * rank];
        let mut right = vec![0.0; rank * inputs];
        for o in 0..outputs {
            for r in 0..rank {
                left[o * rank + r] = tall_right[r * outputs + o];
            }
        }
        for r in 0..rank {
            for i in 0..inputs {
                right[r * inputs + i] = tall_left[i * rank + r];
            }
        }
        (left, right)
    } else {
        (tall_left, tall_right)
    };
    finite(&left).map_err(|_| "SVD factor overflow")?;
    finite(&right).map_err(|_| "SVD factor overflow")?;
    Ok(Decomposition {
        left,
        right,
        singular_values,
        sweeps,
    })
}

pub fn control(source: &Artifact, rank: usize) -> Result<Artifact> {
    source.validate()?;
    if source.representation != "dense" {
        return Err("SVD conversion expects a dense SiLU source".into());
    }
    let [n, w, m] = source.shape;
    let gate = decompose(n, w, rank, &source.gate)?;
    let up = decompose(n, w, rank, &source.up)?;
    let down = decompose(w, m, rank, &source.down)?;
    let result = Artifact {
        descriptor: source.descriptor.clone(),
        bias: source.bias.clone(),
        schema: "nrl.block/5".into(),
        representation: "svd".into(),
        shape: source.shape,
        gate: vec![],
        up: vec![],
        down: vec![],
        spline: None,
        selection: None,
        factors: Some(FactorData {
            rank,
            gate_left: gate.left,
            gate_right: gate.right,
            up_left: up.left,
            up_right: up.right,
            down_left: down.left,
            down_right: down.right,
        }),
    };
    result.validate()?;
    Ok(result)
}
