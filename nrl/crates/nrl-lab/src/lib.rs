#![forbid(unsafe_code)]

use nrl_core::{finite, Shape, MAX_ELEMENTS};
use nrl_repr::{Activation, Block, Gradients, Spline};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_ARTIFACT_BYTES: usize = 1 << 20;
pub const MAX_ARTIFACT_VALUES: usize = 16384;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplineData {
    pub knots: usize,
    pub low: Vec<f64>,
    pub high: Vec<f64>,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionData {
    pub original_channels: usize,
    pub indices: Vec<usize>,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub schema: String,
    pub representation: String,
    pub shape: [usize; 3],
    pub gate: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
    pub spline: Option<SplineData>,
    pub selection: Option<SelectionData>,
}

fn numeric<T>(value: nrl_core::Result<T>) -> Result<T> {
    value.map_err(|e| format!("numerical contract: {e:?}"))
}

impl Artifact {
    pub fn block(&self) -> Result<Block<'_>> {
        if self.schema != "nrl.block/2" {
            return Err(format!("unsupported artifact schema: {}", self.schema));
        }
        let mut total = self
            .gate
            .len()
            .checked_add(self.up.len())
            .and_then(|n| n.checked_add(self.down.len()))
            .ok_or("artifact value count overflow")?;
        if let Some(s) = &self.spline {
            for n in [s.low.len(), s.high.len(), s.values.len()] {
                total = total
                    .checked_add(n)
                    .ok_or("artifact value count overflow")?;
            }
        }
        if total > MAX_ARTIFACT_VALUES {
            return Err("artifact value limit exceeded".into());
        }
        let shape = numeric(Shape::new(self.shape[0], self.shape[1], self.shape[2]))?;
        if let Some(selection) = &self.selection {
            numeric(Shape::new(
                shape.input(),
                selection.original_channels,
                shape.output(),
            ))?;
            if selection.indices.len() != shape.channels()
                || selection
                    .indices
                    .iter()
                    .any(|i| *i >= selection.original_channels)
                || selection.indices.windows(2).any(|pair| pair[0] >= pair[1])
                || !matches!(
                    selection.method.as_str(),
                    "weight-energy-f64/0" | "activation-energy-f64/0"
                )
                || self.representation == "dense"
            {
                return Err("invalid retained-channel mapping".into());
            }
        } else if self.representation == "pruned" {
            return Err("pruned block requires a retained-channel mapping".into());
        }
        let activation = match (self.representation.as_str(), &self.spline) {
            ("dense" | "pruned", None) => Activation::Silu,
            ("spline-linear", Some(s)) => Activation::Spline(numeric(Spline::new(
                shape.channels(),
                s.knots,
                &s.low,
                &s.high,
                &s.values,
            ))?),
            ("spline-cubic", Some(s)) => Activation::Spline(numeric(Spline::hermite(
                shape.channels(),
                s.knots,
                &s.low,
                &s.high,
                &s.values,
            ))?),
            _ => return Err("unsupported representation or inconsistent spline data".into()),
        };
        numeric(Block::new(
            shape, &self.gate, &self.up, &self.down, activation,
        ))
    }

    pub fn validate(&self) -> Result<()> {
        self.block().map(|_| ())
    }

    pub fn forward(&self, input: &[f64]) -> Result<Vec<f64>> {
        let block = self.block()?;
        if input.is_empty() || input.len() > MAX_ELEMENTS || input.len() % self.shape[0] != 0 {
            return Err("invalid input batch".into());
        }
        let rows = input.len() / self.shape[0];
        let count = numeric(nrl_core::elements(rows, self.shape[2]))?;
        let mut output = vec![0.0; count];
        let mut scratch = vec![0.0; self.shape[1] * 3];
        for (x, y) in input
            .chunks_exact(self.shape[0])
            .zip(output.chunks_exact_mut(self.shape[2]))
        {
            numeric(block.forward(x, y, &mut scratch))?;
        }
        Ok(output)
    }

    pub fn step(&self, gradients: &OwnedGradients, rate: f64) -> Result<Self> {
        self.validate()?;
        if !rate.is_finite() || rate <= 0.0 {
            return Err("learning rate must be positive and finite".into());
        }
        let mut next = self.clone();
        fn update(weights: &mut [f64], gradients: &[f64], rate: f64) -> Result<()> {
            if weights.len() != gradients.len() {
                return Err("gradient shape mismatch".into());
            }
            numeric(finite(gradients))?;
            for (w, g) in weights.iter_mut().zip(gradients) {
                *w -= rate * g;
            }
            Ok(())
        }
        update(&mut next.gate, &gradients.gate, rate)?;
        update(&mut next.up, &gradients.up, rate)?;
        update(&mut next.down, &gradients.down, rate)?;
        if let Some(spline) = &mut next.spline {
            update(&mut spline.values, &gradients.values, rate)?;
        } else if !gradients.values.is_empty() {
            return Err("dense block has no spline gradient".into());
        }
        next.validate()?;
        Ok(next)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwnedGradients {
    pub gate: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
    pub values: Vec<f64>,
}

#[derive(Debug)]
pub struct Evaluation {
    pub output: Vec<f64>,
    pub input_gradient: Vec<f64>,
    pub gradients: OwnedGradients,
    pub loss: f64,
}

pub fn evaluate(artifact: &Artifact, input: &[f64], target: &[f64]) -> Result<Evaluation> {
    let output = artifact.forward(input)?;
    if target.len() != output.len() {
        return Err("target shape mismatch".into());
    }
    numeric(finite(target))?;
    let count = output.len() as f64;
    let loss = output
        .iter()
        .zip(target)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        / count;
    numeric(finite(&[loss]))?;
    let mut gradients = OwnedGradients {
        gate: vec![0.0; artifact.gate.len()],
        up: vec![0.0; artifact.up.len()],
        down: vec![0.0; artifact.down.len()],
        values: vec![0.0; artifact.spline.as_ref().map_or(0, |s| s.values.len())],
    };
    let mut input_gradient = vec![0.0; input.len()];
    let mut scratch = vec![0.0; artifact.shape[1] * 3];
    let mut dy = vec![0.0; artifact.shape[2]];
    let block = artifact.block()?;
    for row in 0..input.len() / artifact.shape[0] {
        let xs = row * artifact.shape[0];
        let ys = row * artifact.shape[2];
        for (i, d) in dy.iter_mut().enumerate() {
            *d = 2.0 * (output[ys + i] - target[ys + i]) / count;
        }
        numeric(block.backward(
            &input[xs..xs + artifact.shape[0]],
            &dy,
            &mut input_gradient[xs..xs + artifact.shape[0]],
            &mut Gradients {
                gate: &mut gradients.gate,
                up: &mut gradients.up,
                down: &mut gradients.down,
                values: &mut gradients.values,
            },
            &mut scratch,
        ))?;
    }
    Ok(Evaluation {
        output,
        input_gradient,
        gradients,
        loss,
    })
}

pub fn read_json(path: &Path) -> Result<serde_json::Value> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take((MAX_ARTIFACT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("artifact byte limit exceeded".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<Artifact> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take((MAX_ARTIFACT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("artifact byte limit exceeded".into());
    }
    let artifact: Artifact = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    artifact.validate()?;
    Ok(artifact)
}

pub fn save(path: &Path, artifact: &Artifact) -> Result<()> {
    artifact.validate()?;
    let bytes = serde_json::to_vec_pretty(artifact).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("artifact byte limit exceeded".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?
        .write_all(&bytes)
        .map_err(|e| e.to_string())
}

pub fn close(actual: &[f64], expected: &[f64]) -> Result<f64> {
    if actual.len() != expected.len() {
        return Err("comparison shape mismatch".into());
    }
    numeric(finite(actual))?;
    numeric(finite(expected))?;
    let mut max: f64 = 0.0;
    for (a, b) in actual.iter().zip(expected) {
        let error = (a - b).abs();
        if error > 1e-12 + 1e-10 * b.abs() {
            return Err(format!("reference mismatch: {a} vs {b}"));
        }
        max = max.max(error);
    }
    Ok(max)
}

fn vector(value: &serde_json::Value) -> Result<Vec<f64>> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

pub fn check_case(case: &serde_json::Value) -> Result<serde_json::Value> {
    let artifact: Artifact =
        serde_json::from_value(case["artifact"].clone()).map_err(|e| e.to_string())?;
    let result = evaluate(
        &artifact,
        &vector(&case["input"])?,
        &vector(&case["target"])?,
    )?;
    let expected: OwnedGradients =
        serde_json::from_value(case["gradients"].clone()).map_err(|e| e.to_string())?;
    let error = [
        close(&result.output, &vector(&case["output"])?)?,
        close(&result.input_gradient, &vector(&case["input_gradient"])?)?,
        close(&result.gradients.gate, &expected.gate)?,
        close(&result.gradients.up, &expected.up)?,
        close(&result.gradients.down, &expected.down)?,
        close(&result.gradients.values, &expected.values)?,
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    close(
        &[result.loss],
        &[case["loss"].as_f64().ok_or("missing reference loss")?],
    )?;
    let next = artifact.step(
        &result.gradients,
        case["learning_rate"]
            .as_f64()
            .ok_or("missing learning rate")?,
    )?;
    let after = evaluate(&next, &vector(&case["input"])?, &vector(&case["target"])?)?.loss;
    close(
        &[after],
        &[case["trained_loss"]
            .as_f64()
            .ok_or("missing reference trained loss")?],
    )?;
    if after >= result.loss {
        return Err("fixture training step did not lower loss".into());
    }
    Ok(
        serde_json::json!({"representation": artifact.representation, "max_reference_error": error,
        "loss_before": result.loss, "loss_after": after, "sample_count": vector(&case["input"])?.len() / artifact.shape[0]}),
    )
}

pub fn prune(source: &Artifact, width: usize, calibration: Option<&[f64]>) -> Result<Artifact> {
    let block = source.block()?;
    if source.representation != "dense" {
        return Err("pruning expects a dense SiLU source block".into());
    }
    if width == 0 || width > source.shape[1] {
        return Err("retained width is outside the source channels".into());
    }
    let [n, w, m] = source.shape;
    let mut scores = vec![0.0; w];
    let mut scratch = vec![0.0; w * 3];
    numeric(nrl_repr::pruning::scores(
        &block,
        calibration,
        &mut scores,
        &mut scratch,
    ))?;
    let mut indices = vec![0; width];
    numeric(nrl_repr::pruning::select(&scores, &mut indices))?;
    let gate = indices
        .iter()
        .flat_map(|c| source.gate[c * n..(c + 1) * n].iter().copied())
        .collect();
    let up = indices
        .iter()
        .flat_map(|c| source.up[c * n..(c + 1) * n].iter().copied())
        .collect();
    let down = (0..m)
        .flat_map(|o| indices.iter().map(move |c| source.down[o * w + c]))
        .collect();
    let result = Artifact {
        schema: "nrl.block/2".into(),
        representation: "pruned".into(),
        shape: [n, width, m],
        gate,
        up,
        down,
        spline: None,
        selection: Some(SelectionData {
            original_channels: w,
            indices,
            method: if calibration.is_some() {
                "activation-energy-f64/0".into()
            } else {
                "weight-energy-f64/0".into()
            },
        }),
    };
    result.validate()?;
    Ok(result)
}

// Grid selection and down-projection refitting are separate calibration decisions.
pub fn linear_spline_control(
    pruned: &Artifact,
    knots: usize,
    low: Vec<f64>,
    high: Vec<f64>,
) -> Result<Artifact> {
    pruned.validate()?;
    if pruned.representation != "pruned"
        || knots < 2
        || low.len() != pruned.shape[1]
        || high.len() != low.len()
    {
        return Err("spline control expects a pruned block and a matching grid".into());
    }
    let count = numeric(nrl_core::elements(low.len(), knots))?;
    if count > MAX_ARTIFACT_VALUES {
        return Err("spline coefficient limit exceeded".into());
    }
    let mut values = Vec::with_capacity(count);
    for c in 0..low.len() {
        for k in 0..knots {
            let x = low[c] + (high[c] - low[c]) * (k as f64 / (knots - 1) as f64);
            values.push(nrl_repr::silu(x).0);
        }
    }
    let mut result = pruned.clone();
    result.representation = "spline-linear".into();
    result.spline = Some(SplineData {
        knots,
        low,
        high,
        values,
    });
    result.validate()?;
    Ok(result)
}

pub fn cubic_spline_control(
    pruned: &Artifact,
    knots: usize,
    low: Vec<f64>,
    high: Vec<f64>,
) -> Result<Artifact> {
    let mut result = linear_spline_control(pruned, knots, low, high)?;
    result.representation = "spline-cubic".into();
    result.validate()?;
    Ok(result)
}
