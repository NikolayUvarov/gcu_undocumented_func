#![forbid(unsafe_code)]

use nrl_core::{finite, Shape, MAX_ELEMENTS};
use nrl_repr::factor::{Factor, FactorGradients, SvdBlock, SvdGradients};
use nrl_repr::{Activation, Block, Gradients, Spline};

pub mod decoder;
pub mod descriptor;
pub mod experiment;
mod import_json;
pub mod model;
pub mod qsl;
pub mod research;
pub mod runtime_bridge;
pub mod safetensors;
mod storage;
pub mod svd;
use descriptor::BlockDescriptor;
use nrl_repr::bias::{BiasGradients, Biases};
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
    pub mapping: String,
    pub original_channels: usize,
    pub indices: Vec<usize>,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactorData {
    pub rank: usize,
    pub gate_left: Vec<f64>,
    pub gate_right: Vec<f64>,
    pub up_left: Vec<f64>,
    pub up_right: Vec<f64>,
    pub down_left: Vec<f64>,
    pub down_right: Vec<f64>,
}
impl FactorData {
    pub fn tensors(&self) -> [&[f64]; 6] {
        [
            &self.gate_left,
            &self.gate_right,
            &self.up_left,
            &self.up_right,
            &self.down_left,
            &self.down_right,
        ]
    }
    pub fn tensors_mut(&mut self) -> [&mut [f64]; 6] {
        [
            &mut self.gate_left,
            &mut self.gate_right,
            &mut self.up_left,
            &mut self.up_right,
            &mut self.down_left,
            &mut self.down_right,
        ]
    }
    fn zeros(&self) -> Self {
        let mut result = self.clone();
        for tensor in result.tensors_mut() {
            tensor.fill(0.0);
        }
        result
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiasData {
    pub gate: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
}
impl BiasData {
    pub fn tensors(&self) -> [&[f64]; 3] {
        [&self.gate, &self.up, &self.down]
    }
    pub fn tensors_mut(&mut self) -> [&mut [f64]; 3] {
        [&mut self.gate, &mut self.up, &mut self.down]
    }
    fn zeros(&self) -> Self {
        let mut result = self.clone();
        for t in result.tensors_mut() {
            t.fill(0.0);
        }
        result
    }
    fn view(&self) -> Biases<'_> {
        Biases {
            gate: &self.gate,
            up: &self.up,
            down: &self.down,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub descriptor: BlockDescriptor,
    pub bias: Option<BiasData>,
    pub schema: String,
    pub representation: String,
    pub shape: [usize; 3],
    pub gate: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
    pub spline: Option<SplineData>,
    pub selection: Option<SelectionData>,
    pub factors: Option<FactorData>,
}

fn numeric<T>(value: nrl_core::Result<T>) -> Result<T> {
    value.map_err(|e| format!("numerical contract: {e:?}"))
}

pub enum RuntimeBlock<'a> {
    Compact(Block<'a>),
    Svd(SvdBlock<'a>),
}
impl RuntimeBlock<'_> {
    pub fn scratch_len(&self) -> usize {
        match self {
            Self::Compact(b) => b.scratch_len(),
            Self::Svd(b) => b.scratch_len(),
        }
    }
    pub fn forward(
        &self,
        input: &[f64],
        output: &mut [f64],
        scratch: &mut [f64],
    ) -> nrl_core::Result<()> {
        match self {
            Self::Compact(b) => b.forward(input, output, scratch),
            Self::Svd(b) => b.forward(input, output, scratch),
        }
    }
    pub(crate) fn backward(
        &self,
        input: &[f64],
        dy: &[f64],
        dx: &mut [f64],
        gradients: &mut OwnedGradients,
        scratch: &mut [f64],
    ) -> nrl_core::Result<()> {
        let mut empty_bias = BiasData::default();
        let bias = gradients.bias.as_mut().unwrap_or(&mut empty_bias);
        match self {
            Self::Compact(b) => b.backward(
                input,
                dy,
                dx,
                &mut Gradients {
                    biases: BiasGradients {
                        gate: &mut bias.gate,
                        up: &mut bias.up,
                        down: &mut bias.down,
                    },
                    gate: &mut gradients.gate,
                    up: &mut gradients.up,
                    down: &mut gradients.down,
                    values: &mut gradients.values,
                },
                scratch,
            ),
            Self::Svd(b) => {
                let f = gradients.factors.as_mut().ok_or(nrl_core::Error::Shape)?;
                b.backward(
                    input,
                    dy,
                    dx,
                    &mut SvdGradients {
                        gate: FactorGradients {
                            bias: &mut bias.gate,
                            left: &mut f.gate_left,
                            right: &mut f.gate_right,
                        },
                        up: FactorGradients {
                            bias: &mut bias.up,
                            left: &mut f.up_left,
                            right: &mut f.up_right,
                        },
                        down: FactorGradients {
                            bias: &mut bias.down,
                            left: &mut f.down_left,
                            right: &mut f.down_right,
                        },
                    },
                    scratch,
                )
            }
        }
    }
}

impl Artifact {
    pub fn block(&self) -> Result<RuntimeBlock<'_>> {
        if self.schema != "nrl.block/5" {
            return Err(format!("unsupported artifact schema: {}", self.schema));
        }
        self.descriptor.check("cpu-scalar")?;
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
        if let Some(factors) = &self.factors {
            for tensor in factors.tensors() {
                total = total
                    .checked_add(tensor.len())
                    .ok_or("artifact value count overflow")?;
            }
        }
        if let Some(bias) = &self.bias {
            for tensor in bias.tensors() {
                total = total
                    .checked_add(tensor.len())
                    .ok_or("artifact value count overflow")?;
            }
        }
        if total > MAX_ARTIFACT_VALUES {
            return Err("artifact value limit exceeded".into());
        }
        let shape = numeric(Shape::new(self.shape[0], self.shape[1], self.shape[2]))?;
        let biases = self.bias.as_ref().map_or(Biases::default(), BiasData::view);
        numeric(biases.validate(shape))?;
        if self.representation == "svd" {
            if self.spline.is_some()
                || self.selection.is_some()
                || !self.gate.is_empty()
                || !self.up.is_empty()
                || !self.down.is_empty()
            {
                return Err("factor block has inconsistent dense, spline or selection data".into());
            }
            let f = self.factors.as_ref().ok_or("SVD block requires factors")?;
            let gate = numeric(Factor::new(
                shape.input(),
                shape.channels(),
                f.rank,
                &f.gate_left,
                &f.gate_right,
                biases.gate,
            ))?;
            let up = numeric(Factor::new(
                shape.input(),
                shape.channels(),
                f.rank,
                &f.up_left,
                &f.up_right,
                biases.up,
            ))?;
            let down = numeric(Factor::new(
                shape.channels(),
                shape.output(),
                f.rank,
                &f.down_left,
                &f.down_right,
                biases.down,
            ))?;
            return Ok(RuntimeBlock::Svd(numeric(SvdBlock::new(
                shape, gate, up, down,
            ))?));
        }
        if self.factors.is_some() {
            return Err("compact block cannot carry SVD factors".into());
        }
        if let Some(selection) = &self.selection {
            numeric(Shape::new(
                shape.input(),
                selection.original_channels,
                shape.output(),
            ))?;
            let valid = match selection.mapping.as_str() {
                "known" => {
                    selection.indices.len() == shape.channels()
                        && selection
                            .indices
                            .iter()
                            .all(|i| *i < selection.original_channels)
                        && selection.indices.windows(2).all(|p| p[0] < p[1])
                        && matches!(
                            selection.method.as_str(),
                            "weight-energy-f64/1" | "activation-energy-f64/1"
                        )
                }
                "unrecorded" => {
                    selection.indices.is_empty() && selection.method == "qsl-unrecorded/0"
                }
                _ => false,
            };
            if !valid
                || selection.original_channels < shape.channels()
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
            shape, &self.gate, &self.up, &self.down, activation, biases,
        ))
        .map(RuntimeBlock::Compact)
    }

    pub fn block_on(&self, device: &str) -> Result<RuntimeBlock<'_>> {
        self.descriptor.check(device)?;
        self.block()
    }

    pub fn compact_block(&self) -> Result<Block<'_>> {
        match self.block()? {
            RuntimeBlock::Compact(block) => Ok(block),
            _ => Err("expected a compact block".into()),
        }
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
        let mut scratch = vec![0.0; block.scratch_len()];
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
        match (&mut next.factors, &gradients.factors) {
            (None, None) => (),
            (Some(weights), Some(grads)) if weights.rank == grads.rank => {
                for (w, g) in weights.tensors_mut().into_iter().zip(grads.tensors()) {
                    update(w, g, rate)?;
                }
            }
            _ => return Err("factor gradient mismatch".into()),
        }
        match (&mut next.bias, &gradients.bias) {
            (None, None) => (),
            (Some(weights), Some(grads)) => {
                for (w, g) in weights.tensors_mut().into_iter().zip(grads.tensors()) {
                    update(w, g, rate)?;
                }
            }
            _ => return Err("bias gradient mismatch".into()),
        }
        next.validate()?;
        Ok(next)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedGradients {
    pub bias: Option<BiasData>,
    pub factors: Option<FactorData>,
    pub gate: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
    pub values: Vec<f64>,
}

impl OwnedGradients {
    pub(crate) fn zeros(artifact: &Artifact) -> Self {
        Self {
            bias: artifact.bias.as_ref().map(BiasData::zeros),
            gate: vec![0.0; artifact.gate.len()],
            up: vec![0.0; artifact.up.len()],
            down: vec![0.0; artifact.down.len()],
            values: vec![0.0; artifact.spline.as_ref().map_or(0, |s| s.values.len())],
            factors: artifact.factors.as_ref().map(FactorData::zeros),
        }
    }
    pub fn tensors(&self) -> Vec<&[f64]> {
        let mut tensors: Vec<&[f64]> = vec![&self.gate, &self.up, &self.down, &self.values];
        if let Some(f) = &self.factors {
            tensors.extend(f.tensors());
        }
        if let Some(b) = &self.bias {
            tensors.extend(b.tensors());
        }
        tensors
    }
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
    let mut gradients = OwnedGradients::zeros(artifact);
    let mut input_gradient = vec![0.0; input.len()];
    let mut scratch = vec![0.0; artifact.block()?.scratch_len()];
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
            &mut gradients,
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
    parse(&bytes)
}

pub fn parse(bytes: &[u8]) -> Result<Artifact> {
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("artifact byte limit exceeded".into());
    }
    #[derive(Deserialize)]
    struct Header {
        schema: String,
    }
    let header: Header = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if header.schema != "nrl.block/5" {
        return Err(format!("unsupported artifact schema: {}", header.schema));
    }
    let artifact: Artifact = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
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
    let mut error = error;
    match (&result.gradients.factors, &expected.factors) {
        (None, None) => (),
        (Some(actual), Some(expected)) if actual.rank == expected.rank => {
            for (a, b) in actual.tensors().into_iter().zip(expected.tensors()) {
                error = error.max(close(a, b)?);
            }
        }
        _ => return Err("reference factor gradient mismatch".into()),
    }
    match (&result.gradients.bias, &expected.bias) {
        (None, None) => (),
        (Some(actual), Some(expected)) => {
            for (a, b) in actual.tensors().into_iter().zip(expected.tensors()) {
                error = error.max(close(a, b)?);
            }
        }
        _ => return Err("reference bias gradient mismatch".into()),
    }
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
        "loss_before": result.loss, "loss_after": after, "bias_parameters": artifact.bias.as_ref().map_or(0, |b| b.tensors().iter().map(|t| t.len()).sum::<usize>()), "sample_count": vector(&case["input"])?.len() / artifact.shape[0]}),
    )
}

pub fn prune(source: &Artifact, width: usize, calibration: Option<&[f64]>) -> Result<Artifact> {
    let block = source.compact_block()?;
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
    let bias = source.bias.as_ref().map(|b| BiasData {
        gate: if b.gate.is_empty() {
            vec![]
        } else {
            indices.iter().map(|c| b.gate[*c]).collect()
        },
        up: if b.up.is_empty() {
            vec![]
        } else {
            indices.iter().map(|c| b.up[*c]).collect()
        },
        down: b.down.clone(),
    });
    let result = Artifact {
        descriptor: source.descriptor.clone(),
        bias,
        schema: "nrl.block/5".into(),
        representation: "pruned".into(),
        shape: [n, width, m],
        gate,
        up,
        down,
        spline: None,
        factors: None,
        selection: Some(SelectionData {
            mapping: "known".into(),
            original_channels: w,
            indices,
            method: if calibration.is_some() {
                "activation-energy-f64/1".into()
            } else {
                "weight-energy-f64/1".into()
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
