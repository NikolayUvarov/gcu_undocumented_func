use super::{Activation, Block};
use nrl_core::{finite, Error, Result, MAX_ELEMENTS};

pub const MAX_CALIBRATION_ROWS: usize = 4096;

// Scores use f64 throughout; only the first 4096 calibration rows contribute.
pub fn scores(
    source: &Block<'_>,
    calibration: Option<&[f64]>,
    output: &mut [f64],
    scratch: &mut [f64],
) -> Result<()> {
    let shape = source.shape;
    let n = shape.input();
    let w = shape.channels();
    if !matches!(source.activation, Activation::Silu)
        || output.len() != w
        || scratch.len() != shape.scratch_len()
    {
        return Err(Error::Shape);
    }
    output.fill(0.0);
    if let Some(input) = calibration {
        if input.is_empty() || input.len() % n != 0 || input.len() > MAX_ELEMENTS {
            return Err(Error::Shape);
        }
        let rows = (input.len() / n).min(MAX_CALIBRATION_ROWS);
        for row in input[..rows * n].chunks_exact(n) {
            source.features(row, scratch)?;
            for c in 0..w {
                let feature = scratch[w + c] * scratch[2 * w + c];
                output[c] += feature * feature;
            }
            finite(output)?;
        }
        for score in output.iter_mut() {
            *score /= rows as f64;
        }
    } else {
        for (c, score) in output.iter_mut().enumerate() {
            let mut g: f64 = source.gate[c * n..(c + 1) * n].iter().map(|v| v * v).sum();
            let mut u: f64 = source.up[c * n..(c + 1) * n].iter().map(|v| v * v).sum();
            let bg = crate::bias::value(source.biases.gate, c);
            let bu = crate::bias::value(source.biases.up, c);
            g += bg * bg;
            u += bu * bu;
            finite(&[g, u])?;
            *score = g * u;
        }
        finite(output)?;
    }
    for (c, score) in output.iter_mut().enumerate() {
        let energy: f64 = (0..shape.output())
            .map(|o| {
                let v = source.down[o * w + c];
                v * v
            })
            .sum();
        finite(&[energy])?;
        *score *= energy;
    }
    finite(output)
}

// Ties prefer the lower source index; the returned mapping is ascending.
pub fn select(scores: &[f64], indices: &mut [usize]) -> Result<()> {
    if scores.is_empty()
        || scores.len() > MAX_ELEMENTS
        || indices.is_empty()
        || indices.len() > scores.len()
    {
        return Err(Error::Shape);
    }
    finite(scores)?;
    if scores.iter().any(|s| *s < 0.0) {
        return Err(Error::Shape);
    }
    if indices.len() == scores.len() {
        for (index, slot) in indices.iter_mut().enumerate() {
            *slot = index;
        }
        return Ok(());
    }
    let better = |a: usize, b: usize| scores[a] > scores[b] || (scores[a] == scores[b] && a < b);
    for candidate in 0..scores.len() {
        if candidate < indices.len() {
            indices[candidate] = candidate;
            let mut child = candidate;
            while child > 0 {
                let parent = (child - 1) / 2;
                if !better(indices[parent], indices[child]) {
                    break;
                }
                indices.swap(parent, child);
                child = parent;
            }
        } else if better(candidate, indices[0]) {
            indices[0] = candidate;
            let mut parent = 0;
            while parent * 2 + 1 < indices.len() {
                let mut child = parent * 2 + 1;
                if child + 1 < indices.len() && better(indices[child], indices[child + 1]) {
                    child += 1;
                }
                if !better(indices[parent], indices[child]) {
                    break;
                }
                indices.swap(parent, child);
                parent = child;
            }
        }
    }
    indices.sort_unstable();
    Ok(())
}
