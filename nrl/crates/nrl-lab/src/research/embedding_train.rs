//! Full-batch graded contrastive training over query splits and one shared document index.
use super::*;
use crate::{
    embedding::{Definition, Encoder},
    model::Source,
    optimizer::{self, Tensors},
};
use serde::{Deserialize, Serialize};
use std::fs;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub optimizer: optimizer::Config,
    pub temperature: f64,
    pub max_training_values: usize,
    pub source: Source,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        self.optimizer.validate()?;
        crate::model::validate_sources(std::slice::from_ref(&self.source))?;
        if self.schema != "nrl.embedding-training-config/0"
            || self.optimizer.algorithm != "adamw"
            || !(0.01..=1.0).contains(&self.temperature)
            || !(1..=crate::model::MAX_ACTIVATION_VALUES).contains(&self.max_training_values)
        {
            return Err("embedding training profile/temperature/budget".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let c: Self = parse(&read(path)?, "nrl.embedding-training-config/0")?;
        c.validate()?;
        Ok(c)
    }
}
fn accumulate(a: &mut Tensors, b: &Tensors) -> Result<()> {
    if a.keys().ne(b.keys()) {
        return Err("embedding gradient inventory".into());
    }
    for (k, v) in a {
        if v.len() != b[k].len() {
            return Err("embedding gradient shape".into());
        }
        for (x, y) in v.iter_mut().zip(&b[k]) {
            *x += y;
        }
        finite(v)?;
    }
    Ok(())
}
pub fn objective(
    encoder: &Encoder,
    data: &retrieval::Dataset,
    split: &str,
    config: &Config,
    backward: bool,
) -> Result<(f64, Tensors)> {
    config.validate()?;
    let (documents, queries) = data.tokenize(encoder)?;
    let selected: Vec<_> = data
        .queries
        .iter()
        .zip(&queries)
        .filter(|(q, _)| q.split == split)
        .collect();
    if selected.is_empty() || (backward && split != "train") {
        return Err("embedding fit uses train query split only".into());
    }
    let vectors = documents
        .iter()
        .map(|t| encoder.encode_tokens(t))
        .collect::<Result<Vec<_>>>()?;
    let width = encoder.output_width()?;
    let count = selected.len() as f64;
    let mut doc_dy = vec![vec![0.0; width]; documents.len()];
    let mut loss = 0.0;
    let mut gradients = encoder.parameters()?;
    for v in gradients.values_mut() {
        v.fill(0.0);
    }
    for (query, tokens) in selected {
        let v = encoder.encode_tokens(tokens)?;
        let scores: Vec<_> = vectors
            .iter()
            .map(|d| d.iter().zip(&v).map(|(x, y)| x * y).sum::<f64>() / config.temperature)
            .collect();
        let max = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let sum = scores.iter().map(|s| (s - max).exp()).sum::<f64>();
        let lse = max + sum.ln();
        let total: f64 = query
            .relevance
            .values()
            .map(|g| 2.0f64.powi(*g as i32) - 1.0)
            .sum();
        let mut query_dy = vec![0.0; width];
        for (i, doc) in data.documents.iter().enumerate() {
            let target =
                (2.0f64.powi(*query.relevance.get(&doc.id).unwrap_or(&0) as i32) - 1.0) / total;
            loss += target * (lse - scores[i]) / count;
            let ds = ((scores[i] - max).exp() / sum - target) / (config.temperature * count);
            for j in 0..width {
                query_dy[j] += ds * vectors[i][j];
                doc_dy[i][j] += ds * v[j];
            }
        }
        if backward {
            accumulate(
                &mut gradients,
                &encoder
                    .backward_tokens(tokens, &query_dy, config.max_training_values)?
                    .gradients,
            )?;
        }
    }
    if backward {
        for (tokens, dy) in documents.iter().zip(doc_dy) {
            accumulate(
                &mut gradients,
                &encoder
                    .backward_tokens(tokens, &dy, config.max_training_values)?
                    .gradients,
            )?;
        }
    }
    finite(&[loss])?;
    Ok((loss, gradients))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch {
    pub step: usize,
    pub train_loss: f64,
    pub validation_loss: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema: String,
    pub source_model_id: String,
    pub initial: Definition,
    pub data: retrieval::Dataset,
    pub config: Config,
    pub current: Tensors,
    pub best: Tensors,
    pub optimizer: optimizer::State,
    pub best_step: usize,
    pub history: Vec<Epoch>,
}
impl State {
    pub fn start(encoder: &Encoder, data: retrieval::Dataset, config: Config) -> Result<Self> {
        config.validate()?;
        let p = encoder.parameters()?;
        let history = vec![Epoch {
            step: 0,
            train_loss: objective(encoder, &data, "train", &config, false)?.0,
            validation_loss: objective(encoder, &data, "validation", &config, false)?.0,
        }];
        let s = Self {
            schema: "nrl.embedding-training-state/0".into(),
            source_model_id: encoder.model_id().into(),
            initial: encoder.definition(),
            data,
            config: config.clone(),
            current: p.clone(),
            best: p.clone(),
            optimizer: optimizer::State::start(config.optimizer, &p)?,
            best_step: 0,
            history,
        };
        s.validate()?;
        Ok(s)
    }
    fn model(&self, parameters: &Tensors) -> Result<Encoder> {
        Encoder::from_definition(self.initial.clone())?.with_training_parameters(parameters, &[])
    }
    pub fn validate(&self) -> Result<()> {
        self.config.validate()?;
        self.data.validate()?;
        self.optimizer.validate(&self.current)?;
        if self.schema != "nrl.embedding-training-state/0"
            || !storage::valid_hash(&self.source_model_id)
            || self.config.optimizer != self.optimizer.config
            || self.history.len() != self.optimizer.step + 1
            || self.history.iter().enumerate().any(|(i, h)| {
                h.step != i
                    || !h.train_loss.is_finite()
                    || !h.validation_loss.is_finite()
                    || h.train_loss < 0.0
                    || h.validation_loss < 0.0
            })
        {
            return Err("embedding checkpoint identity/order/loss mismatch".into());
        }
        let best = self.history.iter().fold(0, |b, h| {
            if h.validation_loss < self.history[b].validation_loss {
                h.step
            } else {
                b
            }
        });
        if best != self.best_step {
            return Err("embedding validation selection mismatch".into());
        }
        for (p, i) in [
            (&self.current, self.optimizer.step),
            (&self.best, self.best_step),
        ] {
            let m = self.model(p)?;
            let row = &self.history[i];
            if row.train_loss != objective(&m, &self.data, "train", &self.config, false)?.0
                || row.validation_loss
                    != objective(&m, &self.data, "validation", &self.config, false)?.0
            {
                return Err("embedding checkpoint loss/parameter mismatch".into());
            }
        }
        Ok(())
    }
    pub fn advance(&mut self, until: usize) -> Result<()> {
        self.validate()?;
        if until < self.optimizer.step || until > self.config.optimizer.max_steps {
            return Err("embedding continuation step budget".into());
        }
        while self.optimizer.step < until {
            let (_, gradient) = objective(
                &self.model(&self.current)?,
                &self.data,
                "train",
                &self.config,
                true,
            )?;
            let mut parameters = self.current.clone();
            let mut optimizer = self.optimizer.clone();
            optimizer.update(&mut parameters, &gradient)?;
            let next = self.model(&parameters)?;
            let row = Epoch {
                step: optimizer.step,
                train_loss: objective(&next, &self.data, "train", &self.config, false)?.0,
                validation_loss: objective(&next, &self.data, "validation", &self.config, false)?.0,
            };
            if row.validation_loss < self.history[self.best_step].validation_loss {
                self.best = parameters.clone();
                self.best_step = row.step;
            }
            self.current = parameters;
            self.optimizer = optimizer;
            self.history.push(row);
        }
        self.validate()
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        save(
            path,
            &serde_json::json!({"schema":"nrl.embedding-training-checkpoint/0","payload_sha256":id(self)?,"payload":self}),
        )
    }
    pub fn load(path: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Checkpoint {
            schema: String,
            payload_sha256: String,
            payload: State,
        }
        let c: Checkpoint = parse(&read(path)?, "nrl.embedding-training-checkpoint/0")?;
        if c.schema != "nrl.embedding-training-checkpoint/0" || c.payload_sha256 != id(&c.payload)?
        {
            return Err("embedding checkpoint hash mismatch".into());
        }
        c.payload.validate()?;
        Ok(c.payload)
    }
    pub fn export(&self) -> Result<Encoder> {
        self.validate()?;
        let hash = id(self)?;
        let mut source = self.config.source.clone();
        source.identifier = format!("{}#{}", source.identifier, hash);
        source.sha256 = Some(hash);
        Encoder::from_definition(self.initial.clone())?
            .with_training_parameters(&self.best, &[self.data.source.clone(), source])
    }
}
pub fn run(
    encoder: &Encoder,
    data: retrieval::Dataset,
    config: Config,
    path: &Path,
) -> Result<serde_json::Value> {
    let mut s = State::start(encoder, data, config)?;
    s.advance(s.config.optimizer.max_steps)?;
    fs::create_dir(path).map_err(|e| e.to_string())?;
    s.save(&path.join("checkpoint.json"))?;
    let selected = s.export()?;
    selected.save(&path.join("selected-model"))?;
    let mut reports = std::collections::BTreeMap::new();
    for (name, m) in [("baseline", encoder), ("selected", &selected)] {
        for split in ["train", "validation", "test"] {
            let r = retrieval::evaluate_trained(encoder, m, &s.data, split, None)?;
            r.save(&path.join(format!("{name}-{split}.json")))?;
            reports.insert(format!("{name}-{split}"), r);
        }
    }
    let r = serde_json::json!({"schema":"nrl.embedding-training-report/0","source_model_id":s.source_model_id,"data_id":s.data.data_id()?,"config":s.config,"selected_step":s.best_step,"history":s.history,"reports":reports,"selection":"earliest lowest validation graded contrastive cross entropy, including step zero","test_used_for_selection":false,"document_index_scope":"one shared fixed document index; all documents serve as train negatives; only query splits are held out","training_scope":"all lookup and projection parameters; exact mean and L2 Jacobian; no discrete-token gradient","native_execution":false});
    save(&path.join("summary.json"), &r)?;
    Ok(r)
}
