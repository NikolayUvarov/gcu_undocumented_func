use crate::{
    import_json,
    model::{self, Model, Source},
    storage, Result,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const MAX_ROWS: usize = 4096;
pub const MAX_EPOCHS: usize = 256;
pub const MAX_FORWARD_ROWS: usize = 65536;
pub const MAX_DATA_VALUES: usize = 262144;
const EPSILON: f64 = 1e-12;

fn parse<T: DeserializeOwned>(bytes: &[u8], schema: &str) -> Result<T> {
    let value = import_json::parse(bytes, model::MAX_MODEL_BYTES)?;
    if value["schema"].as_str() != Some(schema) {
        return Err(format!(
            "unsupported experiment schema: {}; expected {schema}",
            value["schema"]
        ));
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if bytes.len() > model::MAX_MODEL_BYTES {
        return Err("experiment JSON byte limit".into());
    }
    Ok(bytes)
}
fn identity<T: Serialize>(value: &T) -> Result<String> {
    let json = serde_json::to_value(value).map_err(|e| e.to_string())?;
    Ok(storage::hash(
        import_json::canonical_compact(&json).as_bytes(),
    ))
}
fn finite(values: &[f64]) -> Result<()> {
    nrl_core::finite(values).map_err(|e| format!("experiment numerical contract: {e:?}"))
}
fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub id: String,
    pub group: String,
    pub split: String,
    pub input: Vec<f64>,
    pub target: Vec<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    pub schema: String,
    pub source: Source,
    pub input_width: usize,
    pub output_width: usize,
    pub samples: Vec<Sample>,
}
struct Batch {
    input: Vec<f64>,
    target: Vec<f64>,
    rows: usize,
}
impl Dataset {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let result: Self = parse(bytes, "nrl.regression-data/0")?;
        result.validate()?;
        Ok(result)
    }
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(&storage::read_regular(path, model::MAX_MODEL_BYTES)?)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != "nrl.regression-data/0" {
            return Err(format!("unsupported dataset schema: {}", self.schema));
        }
        model::validate_sources(std::slice::from_ref(&self.source))?;
        if self.samples.is_empty()
            || self.samples.len() > MAX_ROWS
            || self.input_width == 0
            || self.input_width > 4096
            || self.output_width == 0
            || self.output_width > 4096
            || self
                .samples
                .len()
                .checked_mul(self.input_width + self.output_width)
                .is_none_or(|v| v > MAX_DATA_VALUES)
        {
            return Err("dataset shape or value limit".into());
        }
        let mut ids = BTreeSet::new();
        let mut groups = BTreeMap::new();
        let mut inputs = BTreeMap::new();
        let mut splits = BTreeSet::new();
        for row in &self.samples {
            if !identifier(&row.id)
                || !identifier(&row.group)
                || !ids.insert(&row.id)
                || !matches!(row.split.as_str(), "train" | "validation" | "test")
                || row.input.len() != self.input_width
                || row.target.len() != self.output_width
            {
                return Err("invalid or duplicate sample ID, group, split or shape".into());
            }
            finite(&row.input)?;
            finite(&row.target)?;
            if groups
                .insert(&row.group, &row.split)
                .is_some_and(|s| s != &row.split)
            {
                return Err("related group crosses dataset splits".into());
            }
            let key: Vec<_> = row
                .input
                .iter()
                .map(|x| if *x == 0.0 { 0 } else { x.to_bits() })
                .collect();
            if inputs
                .insert(key, &row.split)
                .is_some_and(|s| s != &row.split)
            {
                return Err("duplicate input crosses dataset splits".into());
            }
            splits.insert(&row.split);
        }
        if splits.len() != 3 {
            return Err("train, validation and test must all be nonempty".into());
        }
        Ok(())
    }
    fn batch(&self, split: &str) -> Batch {
        let rows: Vec<_> = self.samples.iter().filter(|r| r.split == split).collect();
        Batch {
            input: rows.iter().flat_map(|r| r.input.iter().copied()).collect(),
            target: rows.iter().flat_map(|r| r.target.iter().copied()).collect(),
            rows: rows.len(),
        }
    }
    pub fn data_id(&self) -> Result<String> {
        self.validate()?;
        identity(self)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub schema: String,
    pub optimizer: String,
    pub selection: String,
    pub epochs: usize,
    pub learning_rate: f64,
    pub max_training_values: usize,
    pub max_forward_rows: usize,
    pub output_source: Source,
}
impl Run {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let result: Self = parse(bytes, "nrl.regression-run/0")?;
        result.validate()?;
        Ok(result)
    }
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(&storage::read_regular(path, model::MAX_MODEL_BYTES)?)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != "nrl.regression-run/0" {
            return Err(format!("unsupported run schema: {}", self.schema));
        }
        model::validate_sources(std::slice::from_ref(&self.output_source))?;
        if self.optimizer != "full-batch-sgd/0"
            || self.selection != "validation-mse-earliest/0"
            || self.epochs == 0
            || self.epochs > MAX_EPOCHS
            || !self.learning_rate.is_finite()
            || self.learning_rate <= 0.0
            || self.max_training_values == 0
            || self.max_training_values > model::MAX_ACTIVATION_VALUES
            || self.max_forward_rows == 0
            || self.max_forward_rows > MAX_FORWARD_ROWS
            || self.output_source.sha256.is_some()
            || self.output_source.identifier.len() > 1983
        {
            return Err("unsupported optimizer, selection, source hash or run budget".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub rows: usize,
    pub values: usize,
    pub mse: f64,
    pub mae: f64,
    pub max_absolute_error: f64,
    pub target_mean_energy: f64,
    pub nmse: f64,
    pub near_zero_target_energy: bool,
}
fn metrics(output: &[f64], batch: &Batch) -> Result<Metrics> {
    if output.len() != batch.target.len() || output.is_empty() {
        return Err("regression output shape mismatch".into());
    }
    finite(output)?;
    finite(&batch.target)?;
    let count = output.len() as f64;
    let mut mse = 0.0;
    let mut mae = 0.0;
    let mut maximum: f64 = 0.0;
    let mut energy = 0.0;
    for (a, b) in output.iter().zip(&batch.target) {
        let error = (a - b).abs();
        mse += error * error / count;
        mae += error / count;
        maximum = maximum.max(error);
        energy += b * b / count;
    }
    let nmse = mse / energy.max(EPSILON);
    finite(&[mse, mae, maximum, energy, nmse])?;
    Ok(Metrics {
        rows: batch.rows,
        values: output.len(),
        mse,
        mae,
        max_absolute_error: maximum,
        target_mean_energy: energy,
        nmse,
        near_zero_target_energy: energy <= EPSILON,
    })
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Epoch {
    pub epoch: usize,
    pub train: Metrics,
    pub validation: Metrics,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SplitMetrics {
    pub train: Metrics,
    pub validation: Metrics,
    pub test: Metrics,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingRecord {
    pub schema: String,
    pub initial_model_id: String,
    pub dataset_id: String,
    pub run_id: String,
    pub completed_epochs: usize,
    pub selected_epoch: usize,
    pub graph_forward_rows: usize,
    pub before_objects: BTreeMap<String, String>,
    pub after_objects: BTreeMap<String, String>,
    pub history: Vec<Epoch>,
    pub baseline: SplitMetrics,
    pub selected: SplitMetrics,
    pub numerical_policy: String,
    pub optimizer_state: String,
    pub rng: String,
    pub implementation: String,
    pub architecture: String,
    pub os: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub training_record_sha256: String,
    pub initial_model_id: String,
    pub selected_model_id: String,
    pub dataset_id: String,
    pub run_id: String,
    pub completed_epochs: usize,
    pub selected_epoch: usize,
    pub graph_forward_rows: usize,
    pub baseline: SplitMetrics,
    pub selected: SplitMetrics,
    pub native_execution: bool,
    pub limitations: Vec<String>,
}
pub struct Experiment {
    initial: Model,
    pub selected: Model,
    pub dataset: Dataset,
    pub config: Run,
    pub record: TrainingRecord,
    pub report: Report,
}
fn object_hashes(model: &Model) -> BTreeMap<String, String> {
    model
        .manifest()
        .objects
        .iter()
        .map(|(k, v)| (k.clone(), v.sha256.clone()))
        .collect()
}
fn record_source(config: &Run, hash: &str) -> Source {
    let mut source = config.output_source.clone();
    source.identifier.push('#');
    source.identifier.push_str(hash);
    source.sha256 = Some(hash.to_owned());
    source
}
fn check_run(model: &Model, dataset: &Dataset, config: &Run) -> Result<usize> {
    dataset.validate()?;
    config.validate()?;
    let info = model.info()?;
    if model.manifest().kind != "standalone"
        || !model
            .manifest()
            .descriptor
            .tasks
            .iter()
            .any(|t| t == "regression-vector/0")
        || info["input_width"].as_u64() != Some(dataset.input_width as u64)
        || info["output_width"].as_u64() != Some(dataset.output_width as u64)
    {
        return Err("experiment requires a compatible standalone regression model".into());
    }
    model.with_sources(&[
        dataset.source.clone(),
        record_source(config, &"0".repeat(64)),
    ])?;
    let n = |part: &str| dataset.samples.iter().filter(|r| r.split == part).count();
    let (train, validation, test) = (n("train"), n("validation"), n("test"));
    let work = train + validation + config.epochs * (2 * train + validation) + 2 * test;
    if work > config.max_forward_rows {
        return Err("experiment forward-row work budget exceeded".into());
    }
    let per_row = info["activation_values_per_row"]
        .as_u64()
        .ok_or("missing activation bound")? as usize;
    let parameters = info["trainable_values"]
        .as_u64()
        .ok_or("missing parameter count")? as usize;
    if 2 * train * per_row + parameters > config.max_training_values
        || [train, validation, test].iter().any(|rows| {
            *rows > model.manifest().execution.max_batch_rows
                || rows * per_row > model.manifest().execution.max_activation_values
        })
    {
        return Err("experiment batch, activation or training-value budget exceeded".into());
    }
    Ok(work)
}
pub fn execute(initial: &Model, dataset: Dataset, config: Run) -> Result<Experiment> {
    let work = check_run(initial, &dataset, &config)?;
    let train = dataset.batch("train");
    let validation = dataset.batch("validation");
    let mut current = initial.standalone()?;
    let mut best = current.standalone()?;
    let mut selected_epoch = 0;
    let mut train_output = current.forward(&train.input)?;
    let mut history = vec![Epoch {
        epoch: 0,
        train: metrics(&train_output, &train)?,
        validation: metrics(&current.forward(&validation.input)?, &validation)?,
    }];
    let mut best_loss = history[0].validation.mse;
    for epoch in 1..=config.epochs {
        let dy: Vec<_> = train_output
            .iter()
            .zip(&train.target)
            .map(|(a, b)| 2.0 * (a - b) / train_output.len() as f64)
            .collect();
        finite(&dy)?;
        let backward = current.backward(&train.input, &dy, config.max_training_values)?;
        let parameters = current.sgd_parameters(&backward.gradients, config.learning_rate)?;
        current = current.with_parameters(parameters, &[])?;
        train_output = current.forward(&train.input)?;
        let row = Epoch {
            epoch,
            train: metrics(&train_output, &train)?,
            validation: metrics(&current.forward(&validation.input)?, &validation)?,
        };
        if row.validation.mse < best_loss {
            best_loss = row.validation.mse;
            selected_epoch = epoch;
            best = current.standalone()?;
        }
        history.push(row);
    }
    // Test targets are read for metrics only after checkpoint selection is complete.
    let test = dataset.batch("test");
    let baseline = SplitMetrics {
        train: history[0].train.clone(),
        validation: history[0].validation.clone(),
        test: metrics(&initial.forward(&test.input)?, &test)?,
    };
    let selected = SplitMetrics {
        train: history[selected_epoch].train.clone(),
        validation: history[selected_epoch].validation.clone(),
        test: metrics(&best.forward(&test.input)?, &test)?,
    };
    let record = TrainingRecord {
        schema: "nrl.training-record/0".into(),
        initial_model_id: initial.model_id().into(),
        dataset_id: dataset.data_id()?,
        run_id: identity(&config)?,
        completed_epochs: config.epochs,
        selected_epoch,
        graph_forward_rows: work,
        before_objects: object_hashes(initial),
        after_objects: object_hashes(&best),
        history,
        baseline,
        selected,
        numerical_policy: "cpu-scalar-f64-mse/0; nmse epsilon=1e-12; relu derivative at zero=0"
            .into(),
        optimizer_state: "plain full-batch SGD; no momentum; no resume reader".into(),
        rng: "none; fixed supplied training row order; no shuffling".into(),
        implementation: format!("nrl-lab/{}", env!("CARGO_PKG_VERSION")),
        architecture: std::env::consts::ARCH.into(),
        os: std::env::consts::OS.into(),
    };
    let record_hash = storage::hash(&encode(&record)?);
    let best =
        best.with_sources(&[dataset.source.clone(), record_source(&config, &record_hash)])?;
    let report = Report {
        schema: "nrl.regression-report/0".into(),
        training_record_sha256: record_hash,
        initial_model_id: initial.model_id().into(),
        selected_model_id: best.model_id().into(),
        dataset_id: record.dataset_id.clone(),
        run_id: record.run_id.clone(),
        completed_epochs: record.completed_epochs,
        selected_epoch: record.selected_epoch,
        graph_forward_rows: record.graph_forward_rows,
        baseline: record.baseline.clone(),
        selected: record.selected.clone(),
        native_execution: false,
        limitations: vec![
            "regression only; full-batch scalar CPU f64".into(),
            "no optimizer resume, local calibration/distillation, classification or language task".into(),
            "source declarations retained; no authenticity or licence verification".into(),
            "logical training buffers and forward-row work counts exclude scratch, serialization, parsed objects and host overhead".into(),
            "metrics on supplied data only; no speed, memory, compression or native guarantee".into(),
        ],
    };
    Ok(Experiment {
        initial: initial.standalone()?,
        selected: best,
        dataset,
        config,
        record,
        report,
    })
}
impl Experiment {
    pub fn save(&self, path: &Path) -> Result<()> {
        let files = [
            ("data.json", encode(&self.dataset)?),
            ("run.json", encode(&self.config)?),
            ("training-record.json", encode(&self.record)?),
            ("report.json", encode(&self.report)?),
        ];
        fs::create_dir(path).map_err(|e| e.to_string())?;
        self.initial.save(&path.join("initial-model"))?;
        self.selected.save(&path.join("selected-model"))?;
        for (name, bytes) in files {
            storage::write_new(&path.join(name), &bytes)?;
        }
        Ok(())
    }
}
fn verify_metrics(recorded: &Metrics, actual: &Metrics) -> Result<()> {
    if recorded.rows != actual.rows
        || recorded.values != actual.values
        || recorded.near_zero_target_energy != actual.near_zero_target_energy
    {
        return Err("experiment metric shape or energy flag mismatch".into());
    }
    crate::close(
        &[
            recorded.mse,
            recorded.mae,
            recorded.max_absolute_error,
            recorded.target_mean_energy,
            recorded.nmse,
        ],
        &[
            actual.mse,
            actual.mae,
            actual.max_absolute_error,
            actual.target_mean_energy,
            actual.nmse,
        ],
    )?;
    Ok(())
}
pub fn check_saved(path: &Path) -> Result<Report> {
    let dataset = Dataset::load(&path.join("data.json"))?;
    let config = Run::load(&path.join("run.json"))?;
    let initial = model::load(&path.join("initial-model"), None)?;
    let selected = model::load(&path.join("selected-model"), None)?;
    let record_bytes =
        storage::read_regular(&path.join("training-record.json"), model::MAX_MODEL_BYTES)?;
    let record: TrainingRecord = parse(&record_bytes, "nrl.training-record/0")?;
    let report: Report = parse(
        &storage::read_regular(&path.join("report.json"), model::MAX_MODEL_BYTES)?,
        "nrl.regression-report/0",
    )?;
    let hash = storage::hash(&record_bytes);
    let sources = initial.with_sources(&[dataset.source.clone(), record_source(&config, &hash)])?;
    let best_epoch = record
        .history
        .iter()
        .min_by(|a, b| a.validation.mse.total_cmp(&b.validation.mse))
        .ok_or("empty training history")?
        .epoch;
    if report.native_execution
        || report.training_record_sha256 != hash
        || report.initial_model_id != initial.model_id()
        || report.selected_model_id != selected.model_id()
        || report.dataset_id != dataset.data_id()?
        || report.run_id != identity(&config)?
        || record.initial_model_id != report.initial_model_id
        || record.dataset_id != report.dataset_id
        || record.run_id != report.run_id
        || report.completed_epochs != record.completed_epochs
        || report.selected_epoch != record.selected_epoch
        || report.graph_forward_rows != record.graph_forward_rows
        || report.baseline != record.baseline
        || report.selected != record.selected
        || record.completed_epochs != config.epochs
        || record.graph_forward_rows != check_run(&initial, &dataset, &config)?
        || record.history.len() != config.epochs + 1
        || record.history.iter().enumerate().any(|(i, e)| i != e.epoch)
        || record.selected_epoch != best_epoch
        || best_epoch > config.epochs
        || record.before_objects != object_hashes(&initial)
        || record.after_objects != object_hashes(&selected)
        || initial.manifest().descriptor != selected.manifest().descriptor
        || initial.manifest().execution != selected.manifest().execution
        || initial.manifest().history != selected.manifest().history
        || sources.manifest().sources != selected.manifest().sources
    {
        return Err("experiment identity, selection, provenance or work record mismatch".into());
    }
    for (split, baseline, chosen) in [
        ("train", &record.baseline.train, &record.selected.train),
        (
            "validation",
            &record.baseline.validation,
            &record.selected.validation,
        ),
        ("test", &record.baseline.test, &record.selected.test),
    ] {
        let batch = dataset.batch(split);
        verify_metrics(baseline, &metrics(&initial.forward(&batch.input)?, &batch)?)?;
        verify_metrics(chosen, &metrics(&selected.forward(&batch.input)?, &batch)?)?;
    }
    verify_metrics(&record.history[0].train, &record.baseline.train)?;
    verify_metrics(&record.history[0].validation, &record.baseline.validation)?;
    verify_metrics(&record.history[best_epoch].train, &record.selected.train)?;
    verify_metrics(
        &record.history[best_epoch].validation,
        &record.selected.validation,
    )?;
    Ok(report)
}
