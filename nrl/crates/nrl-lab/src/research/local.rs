use super::*;
use crate::{
    decoder::Decoder,
    model::{ObjectData, Source},
    Artifact, SelectionData,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub schema: String,
    pub representation: String,
    pub width: usize,
    pub knots: usize,
    pub rank: usize,
}
impl Transform {
    pub fn validate(&self) -> Result<()> {
        let valid = match self.representation.as_str() {
            "pruned" => self.knots == 0 && self.rank == 0,
            "spline-linear" | "spline-cubic" => (2..=32).contains(&self.knots) && self.rank == 0,
            "svd" => self.knots == 0 && self.rank > 0 && self.rank <= self.width,
            _ => false,
        };
        if self.schema != "nrl.transform/0" || self.width == 0 || self.width > 64 || !valid {
            return Err("unsupported transform profile/width/grid/rank".into());
        }
        Ok(())
    }
}
pub fn initialize(
    teacher: &Decoder,
    cal: &Calibration,
    spec: &Transform,
) -> Result<(Artifact, Vec<usize>)> {
    spec.validate()?;
    cal.verify_teacher(teacher)?;
    let source = teacher.dense(cal.layer)?;
    if spec.width > source.hidden {
        return Err("transform width exceeds source".into());
    }
    if 3 * source.dim * spec.width
        + if spec.representation.starts_with("spline") {
            spec.width * (spec.knots + 2)
        } else {
            0
        }
        > crate::MAX_ARTIFACT_VALUES
        || (spec.representation == "svd"
            && (source.dim > 64
                || 3 * spec.rank * (source.dim + spec.width) > crate::MAX_ARTIFACT_VALUES))
    {
        return Err("transform intermediate/artifact/SVD profile bound".into());
    }
    let (input, _) = cal.batch("train")?;
    let block = source.block()?;
    let mut scores = vec![0.0; source.hidden];
    let mut scratch = vec![0.0; block.scratch_len()];
    nrl_repr::pruning::scores(&block, Some(&input), &mut scores, &mut scratch)
        .map_err(|e| format!("channel score: {e:?}"))?;
    let mut indices = vec![0; spec.width];
    nrl_repr::pruning::select(&scores, &mut indices)
        .map_err(|e| format!("channel selection: {e:?}"))?;
    let n = source.dim;
    let w = source.hidden;
    let pruned = Artifact {
        schema: "nrl.block/5".into(),
        descriptor: source.descriptor(),
        representation: "pruned".into(),
        shape: [n, spec.width, n],
        bias: None,
        gate: indices
            .iter()
            .flat_map(|c| source.gate[c * n..(c + 1) * n].iter().copied())
            .collect(),
        up: indices
            .iter()
            .flat_map(|c| source.up[c * n..(c + 1) * n].iter().copied())
            .collect(),
        down: (0..n)
            .flat_map(|o| indices.iter().map(move |c| source.down[o * w + c]))
            .collect(),
        spline: None,
        factors: None,
        selection: Some(SelectionData {
            mapping: "known".into(),
            original_channels: w,
            indices: indices.clone(),
            method: "activation-energy-f64/1".into(),
        }),
    };
    pruned.validate()?;
    let result = match spec.representation.as_str() {
        "pruned" => pruned,
        "svd" => {
            let mut dense = pruned;
            dense.representation = "dense".into();
            dense.selection = None;
            crate::svd::control(&dense, spec.rank)?
        }
        _ => {
            let mut low = vec![f64::INFINITY; spec.width];
            let mut high = vec![f64::NEG_INFINITY; spec.width];
            for x in input.chunks_exact(n) {
                for c in 0..spec.width {
                    let gate = pruned.gate[c * n..(c + 1) * n]
                        .iter()
                        .zip(x)
                        .map(|(a, b)| a * b)
                        .sum::<f64>();
                    low[c] = low[c].min(gate);
                    high[c] = high[c].max(gate);
                }
            }
            for c in 0..spec.width {
                let margin = ((high[c] - low[c]) * 0.05).max(1e-6);
                low[c] -= margin;
                high[c] += margin;
            }
            if spec.representation == "spline-linear" {
                crate::linear_spline_control(&pruned, spec.knots, low, high)?
            } else {
                crate::cubic_spline_control(&pruned, spec.knots, low, high)?
            }
        }
    };
    Ok((result, indices))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalConfig {
    pub schema: String,
    pub transform: Transform,
    pub max_steps: usize,
    pub learning_rate: f64,
    pub output_source: Source,
}
impl LocalConfig {
    pub fn validate(&self) -> Result<()> {
        self.transform.validate()?;
        crate::model::validate_sources(std::slice::from_ref(&self.output_source))?;
        if self.schema != "nrl.local-config/0"
            || self.max_steps == 0
            || self.max_steps > 64
            || !self.learning_rate.is_finite()
            || self.learning_rate <= 0.0
            || self.output_source.sha256.is_some()
            || self.output_source.identifier.len() > 1983
        {
            return Err("local training config/rate/step/source limit".into());
        }
        Ok(())
    }
    pub fn load(p: &Path) -> Result<Self> {
        let c: Self = parse(&read(p)?, "nrl.local-config/0")?;
        c.validate()?;
        Ok(c)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LocalMetrics {
    pub values: usize,
    pub mse: f64,
    pub nmse: f64,
    pub mae: f64,
    pub max_absolute_error: f64,
    pub target_mean_energy: f64,
    pub near_zero_energy: bool,
    pub outside_grid_fraction: Option<f64>,
}
pub fn metrics(artifact: &Artifact, input: &[f64], target: &[f64]) -> Result<LocalMetrics> {
    let out = artifact.forward(input)?;
    if out.len() != target.len() || out.is_empty() {
        return Err("local metric target shape".into());
    }
    finite(target)?;
    let count = out.len() as f64;
    let mut mse = 0.0;
    let mut mae = 0.0;
    let mut maximum: f64 = 0.0;
    let mut energy = 0.0;
    for (a, b) in out.iter().zip(target) {
        let error = (a - b).abs();
        mse += error * error / count;
        mae += error / count;
        maximum = maximum.max(error);
        energy += b * b / count;
    }
    let nmse = mse / energy.max(1e-12);
    finite(&[mse, mae, maximum, energy, nmse])?;
    let grid = if let Some(s) = &artifact.spline {
        let [n, w, _] = artifact.shape;
        let mut outside = 0;
        for x in input.chunks_exact(n) {
            for c in 0..w {
                let g = artifact.gate[c * n..(c + 1) * n]
                    .iter()
                    .zip(x)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    + artifact
                        .bias
                        .as_ref()
                        .filter(|b| !b.gate.is_empty())
                        .map_or(0.0, |b| b.gate[c]);
                finite(&[g])?;
                if g < s.low[c] || g > s.high[c] {
                    outside += 1;
                }
            }
        }
        Some(outside as f64 / (input.len() / n * w) as f64)
    } else {
        None
    };
    Ok(LocalMetrics {
        values: out.len(),
        mse,
        nmse,
        mae,
        max_absolute_error: maximum,
        target_mean_energy: energy,
        near_zero_energy: energy <= 1e-12,
        outside_grid_fraction: grid,
    })
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LocalEpoch {
    pub step: usize,
    pub train: LocalMetrics,
    pub validation: LocalMetrics,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalState {
    pub schema: String,
    pub calibration: Calibration,
    pub calibration_id: String,
    pub config: LocalConfig,
    pub config_id: String,
    pub source_hidden: usize,
    pub retained_indices: Vec<usize>,
    pub initial: Artifact,
    pub current: Artifact,
    pub best: Artifact,
    pub step: usize,
    pub best_step: usize,
    pub history: Vec<LocalEpoch>,
    pub optimizer: String,
    pub rng_state: String,
    pub data_order: Vec<String>,
    pub next_data_cursor: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    schema: String,
    payload_sha256: String,
    payload: LocalState,
}
impl LocalState {
    pub fn start(teacher: &Decoder, calibration: Calibration, config: LocalConfig) -> Result<Self> {
        config.validate()?;
        calibration.verify_teacher(teacher)?;
        let mut sources = teacher.sources().to_vec();
        if !sources.contains(&calibration.corpus.source) {
            sources.push(calibration.corpus.source.clone());
        }
        if sources.len() >= 32 {
            return Err("local training needs a derived provenance source slot".into());
        }
        let (initial, retained_indices) = initialize(teacher, &calibration, &config.transform)?;
        let (train, target) = calibration.batch("train")?;
        let (val, val_target) = calibration.batch("validation")?;
        let history = vec![LocalEpoch {
            step: 0,
            train: metrics(&initial, &train, &target)?,
            validation: metrics(&initial, &val, &val_target)?,
        }];
        let state = Self {
            schema: "nrl.local-state/0".into(),
            calibration_id: calibration.calibration_id()?,
            config_id: id(&config)?,
            source_hidden: teacher.dense(calibration.layer)?.hidden,
            retained_indices,
            initial: initial.clone(),
            current: initial.clone(),
            best: initial,
            step: 0,
            best_step: 0,
            history,
            optimizer: "full-batch-sgd/0".into(),
            rng_state:
                "none; deterministic source initialization and supplied document/token order".into(),
            data_order: calibration
                .train
                .iter()
                .map(|p| p.document_id.clone())
                .collect(),
            next_data_cursor: 0,
            calibration,
            config,
        };
        state.validate()?;
        Ok(state)
    }
    pub fn validate(&self) -> Result<()> {
        self.calibration.validate()?;
        self.config.validate()?;
        if self.schema != "nrl.local-state/0"
            || self.calibration_id != self.calibration.calibration_id()?
            || self.config_id != id(&self.config)?
            || self.optimizer != "full-batch-sgd/0"
            || self.rng_state
                != "none; deterministic source initialization and supplied document/token order"
            || self.next_data_cursor != 0
            || self.data_order
                != self
                    .calibration
                    .train
                    .iter()
                    .map(|p| p.document_id.clone())
                    .collect::<Vec<_>>()
            || self.step > self.config.max_steps
            || self.best_step > self.step
            || self.history.len() != self.step + 1
            || self.history.iter().enumerate().any(|(i, h)| i != h.step)
            || self.retained_indices.len() != self.config.transform.width
            || self
                .retained_indices
                .iter()
                .any(|i| *i >= self.source_hidden)
            || self.retained_indices.windows(2).any(|v| v[0] >= v[1])
            || self.source_hidden > 512
        {
            return Err("local state/config/data/order/step identity mismatch".into());
        }
        for row in &self.history {
            for metrics in [&row.train, &row.validation] {
                finite(&[
                    metrics.mse,
                    metrics.nmse,
                    metrics.mae,
                    metrics.max_absolute_error,
                    metrics.target_mean_energy,
                ])?;
                if metrics.values == 0
                    || metrics.mse < 0.0
                    || metrics.nmse < 0.0
                    || metrics.mae < 0.0
                    || metrics.target_mean_energy < 0.0
                    || metrics
                        .outside_grid_fraction
                        .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                {
                    return Err("invalid local history metric".into());
                }
            }
        }
        if self.initial.selection.as_ref().is_some_and(|selection| {
            selection.indices != self.retained_indices
                || selection.original_channels != self.source_hidden
        }) {
            return Err("local retained-channel provenance mismatch".into());
        }
        let initial = ObjectData::Gated(self.initial.clone()).summary()?;
        if self.initial.representation != self.config.transform.representation
            || self.initial.shape
                != [
                    self.calibration.dim,
                    self.config.transform.width,
                    self.calibration.dim,
                ]
            || self.initial.factors.as_ref().map_or(0, |f| f.rank) != self.config.transform.rank
            || self.initial.spline.as_ref().map_or(0, |s| s.knots) != self.config.transform.knots
        {
            return Err("local initialization transform geometry mismatch".into());
        }
        for a in [&self.current, &self.best] {
            if ObjectData::Gated(a.clone()).summary()? != initial
                || serde_json::to_value(&a.selection).unwrap()
                    != serde_json::to_value(&self.initial.selection).unwrap()
                || a.spline.as_ref().map(|s| (&s.low, &s.high))
                    != self.initial.spline.as_ref().map(|s| (&s.low, &s.high))
            {
                return Err("local checkpoint changed fixed geometry/grid/mapping".into());
            }
        }
        let best = self.history.iter().fold(0, |best, row| {
            if row.validation.mse < self.history[best].validation.mse {
                row.step
            } else {
                best
            }
        });
        if best != self.best_step {
            return Err("local checkpoint selection mismatch".into());
        }
        let (train, target) = self.calibration.batch("train")?;
        let (val, val_target) = self.calibration.batch("validation")?;
        for (step, a) in [
            (0, &self.initial),
            (self.step, &self.current),
            (self.best_step, &self.best),
        ] {
            let row = &self.history[step];
            if row.train != metrics(a, &train, &target)?
                || row.validation != metrics(a, &val, &val_target)?
            {
                return Err("local checkpoint metric/parameter mismatch".into());
            }
        }
        Ok(())
    }
    pub fn advance(&mut self, until: usize) -> Result<()> {
        self.validate()?;
        if until < self.step || until > self.config.max_steps {
            return Err("local continuation step budget".into());
        }
        let (train, target) = self.calibration.batch("train")?;
        let (val, val_target) = self.calibration.batch("validation")?;
        while self.step < until {
            let evaluation = crate::evaluate(&self.current, &train, &target)?;
            let next = self
                .current
                .step(&evaluation.gradients, self.config.learning_rate)?;
            let row = LocalEpoch {
                step: self.step + 1,
                train: metrics(&next, &train, &target)?,
                validation: metrics(&next, &val, &val_target)?,
            };
            if row.validation.mse < self.history[self.best_step].validation.mse {
                self.best = next.clone();
                self.best_step = row.step;
            }
            self.current = next;
            self.step = row.step;
            self.history.push(row);
        }
        self.validate()
    }
    pub fn checkpoint_id(&self) -> Result<String> {
        self.validate()?;
        id(self)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        save(
            path,
            &Checkpoint {
                schema: "nrl.local-checkpoint/0".into(),
                payload_sha256: self.checkpoint_id()?,
                payload: self.clone(),
            },
        )
    }
    pub fn load(path: &Path) -> Result<Self> {
        let saved: Checkpoint = parse(&read(path)?, "nrl.local-checkpoint/0")?;
        if saved.payload_sha256 != id(&saved.payload)? {
            return Err("local checkpoint payload hash mismatch".into());
        }
        saved.payload.validate()?;
        Ok(saved.payload)
    }
    pub fn source_record(&self) -> Result<Source> {
        self.validate()?;
        let hash = self.checkpoint_id()?;
        let mut source = self.config.output_source.clone();
        source.identifier.push('#');
        source.identifier.push_str(&hash);
        source.sha256 = Some(hash);
        Ok(source)
    }
    pub fn export(&self, teacher: &Decoder) -> Result<Decoder> {
        self.calibration.verify_teacher(teacher)?;
        let source = self.source_record()?;
        let mut patches = teacher.patches();
        patches.insert(self.calibration.layer, self.best.clone());
        teacher.with_patches(patches, &[self.calibration.corpus.source.clone(), source])
    }
    pub fn report(&self) -> Result<serde_json::Value> {
        self.validate()?;
        Ok(
            serde_json::json!({"schema":"nrl.local-report/0","checkpoint_id":self.checkpoint_id()?,"teacher_model_id":self.calibration.teacher_model_id,"calibration_id":self.calibration_id,"corpus_id":self.calibration.corpus_id,"config_id":self.config_id,"layer":self.calibration.layer,"transform":self.config.transform,"source_hidden":self.source_hidden,"parameter_summary":ObjectData::Gated(self.best.clone()).summary()?,"retained_indices":self.retained_indices,"completed_steps":self.step,"selected_step":self.best_step,"initial":self.history[0],"selected":self.history[self.best_step],"optimizer":self.optimizer,"rng_state":self.rng_state,"next_data_cursor":self.next_data_cursor,"exact_resume_profile":"same scalar f64 executable/host, stored data and full-batch order","native_execution":false}),
        )
    }
}
