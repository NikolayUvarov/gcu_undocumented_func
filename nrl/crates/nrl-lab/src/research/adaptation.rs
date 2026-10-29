//! Resumable local recovery on refreshed student trajectories, not full decoder backpropagation.
use super::*;
use crate::{
    decoder::Decoder,
    model::Source,
    optimizer::{self, Tensors},
    Artifact,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub optimizer: optimizer::Config,
    pub row_limit: usize,
    pub max_validation_nll_increase: f64,
    pub max_package_ratio: f64,
    pub source: Source,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        self.optimizer.validate()?;
        crate::model::validate_sources(std::slice::from_ref(&self.source))?;
        if self.schema != "nrl.adaptation-config/0"
            || !(1..=256).contains(&self.row_limit)
            || !self.max_validation_nll_increase.is_finite()
            || self.max_validation_nll_increase < 0.0
            || !self.max_package_ratio.is_finite()
            || self.max_package_ratio <= 0.0
            || self.max_package_ratio > 4.0
        {
            return Err("adaptation config/row/quality/package budget".into());
        }
        Ok(())
    }
    pub fn load(p: &Path) -> Result<Self> {
        let c: Self = parse(&read(p)?, "nrl.adaptation-config/0")?;
        c.validate()?;
        Ok(c)
    }
}
fn parameters(patches: &BTreeMap<usize, Artifact>) -> Result<Tensors> {
    let mut t = Tensors::new();
    for (layer, a) in patches {
        optimizer::insert(
            &mut t,
            &format!("l{layer}"),
            optimizer::block_parameters(a)?,
        );
    }
    Ok(t)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema: String,
    pub teacher_id: String,
    pub initial_model_id: String,
    pub corpus: Corpus,
    pub config: Config,
    pub sources: Vec<Source>,
    pub initial: BTreeMap<usize, Artifact>,
    pub current: BTreeMap<usize, Artifact>,
    pub best: BTreeMap<usize, Artifact>,
    pub optimizer: optimizer::State,
    pub validation_nll: Vec<f64>,
    pub pair_ids: Vec<String>,
    pub best_step: usize,
}
impl State {
    pub fn start(
        teacher: &Decoder,
        initial: &Decoder,
        corpus: Corpus,
        config: Config,
    ) -> Result<Self> {
        config.validate()?;
        corpus.tokenize(teacher)?;
        if !teacher.patches().is_empty()
            || teacher.base_content_id()? != initial.base_content_id()?
            || !(2..=3).contains(&initial.patches().len())
        {
            return Err(
                "adaptation requires exact dense teacher and two or three student replacements"
                    .into(),
            );
        }
        let current = initial.patches();
        let validation = evaluate::language_quality(initial, &corpus, "validation")?
            .0
            .value;
        let state = Self {
            schema: "nrl.adaptation-state/0".into(),
            teacher_id: teacher.model_id().into(),
            initial_model_id: initial.model_id().into(),
            corpus,
            config: config.clone(),
            sources: initial.sources().to_vec(),
            initial: current.clone(),
            best: current.clone(),
            optimizer: optimizer::State::start(config.optimizer, &parameters(&current)?)?,
            current,
            validation_nll: vec![validation],
            pair_ids: Vec::new(),
            best_step: 0,
        };
        state.validate(teacher)?;
        Ok(state)
    }
    fn model(&self, teacher: &Decoder, patches: &BTreeMap<usize, Artifact>) -> Result<Decoder> {
        teacher.with_patches(patches.clone(), &self.sources)
    }
    pub fn validate(&self, teacher: &Decoder) -> Result<()> {
        self.config.validate()?;
        self.corpus.validate()?;
        self.optimizer.validate(&parameters(&self.current)?)?;
        if self.schema != "nrl.adaptation-state/0"
            || self.teacher_id != teacher.model_id()
            || !teacher.patches().is_empty()
            || self.optimizer.config != self.config.optimizer
            || !(2..=3).contains(&self.initial.len())
            || self.validation_nll.len() != self.optimizer.step + 1
            || self.pair_ids.len() != self.optimizer.step
            || self.pair_ids.iter().any(|s| !storage::valid_hash(s))
            || self
                .validation_nll
                .iter()
                .any(|x| !x.is_finite() || *x < 0.0)
            || self.initial.keys().ne(self.current.keys())
            || self.initial.keys().ne(self.best.keys())
            || self.model(teacher, &self.initial)?.model_id() != self.initial_model_id
        {
            return Err("adaptation checkpoint identity/order/state mismatch".into());
        }
        let best = self.validation_nll.iter().enumerate().fold(0, |b, (i, v)| {
            if *v < self.validation_nll[b] {
                i
            } else {
                b
            }
        });
        if best != self.best_step {
            return Err("adaptation validation selection mismatch".into());
        }
        for patches in [&self.current, &self.best] {
            for (l, a) in patches {
                let init = &self.initial[l];
                if a.shape != init.shape
                    || a.representation != init.representation
                    || serde_json::to_value(&a.selection).unwrap()
                        != serde_json::to_value(&init.selection).unwrap()
                    || a.spline.as_ref().map(|s| (&s.low, &s.high))
                        != init.spline.as_ref().map(|s| (&s.low, &s.high))
                {
                    return Err("adaptation fixed geometry/grid changed".into());
                }
                a.validate()?;
            }
        }
        for (patches, step) in [
            (&self.current, self.optimizer.step),
            (&self.best, self.best_step),
        ] {
            let loss = evaluate::language_quality(
                &self.model(teacher, patches)?,
                &self.corpus,
                "validation",
            )?
            .0
            .value;
            if loss != self.validation_nll[step] {
                return Err("adaptation checkpoint metric/parameter mismatch".into());
            }
        }
        Ok(())
    }
    pub fn advance(&mut self, teacher: &Decoder, until: usize) -> Result<()> {
        self.validate(teacher)?;
        if until < self.optimizer.step || until > self.config.optimizer.max_steps {
            return Err("adaptation continuation budget".into());
        }
        while self.optimizer.step < until {
            let student = self.model(teacher, &self.current)?;
            let pairs = recovery::collect(
                teacher,
                &student,
                &self.corpus,
                self.config.row_limit,
                "frozen-teacher-on-student-input/0",
            )?;
            let mut grads = Tensors::new();
            for (layer, a) in &self.current {
                let (x, y) = pairs.batch(*layer)?;
                let e = crate::evaluate(a, &x, &y)?;
                optimizer::insert(
                    &mut grads,
                    &format!("l{layer}"),
                    optimizer::block_gradients(&e.gradients),
                );
            }
            let mut p = parameters(&self.current)?;
            let mut opt = self.optimizer.clone();
            opt.update(&mut p, &grads)?;
            let next = self
                .current
                .iter()
                .map(|(l, a)| {
                    Ok((
                        *l,
                        optimizer::replace_block(
                            a,
                            &optimizer::take(
                                &p,
                                &format!("l{l}"),
                                optimizer::block_parameters(a)?.len(),
                            )?,
                        )?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?;
            let loss = evaluate::language_quality(
                &self.model(teacher, &next)?,
                &self.corpus,
                "validation",
            )?
            .0
            .value;
            if loss < self.validation_nll[self.best_step] {
                self.best = next.clone();
                self.best_step = opt.step;
            }
            self.current = next;
            self.optimizer = opt;
            self.validation_nll.push(loss);
            self.pair_ids.push(pairs.pair_id()?);
        }
        self.validate(teacher)
    }
    pub fn save(&self, teacher: &Decoder, path: &Path) -> Result<()> {
        self.validate(teacher)?;
        save(
            path,
            &serde_json::json!({"schema":"nrl.adaptation-checkpoint/0","payload_sha256":id(self)?,"payload":self}),
        )
    }
    pub fn load(teacher: &Decoder, path: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Checkpoint {
            schema: String,
            payload_sha256: String,
            payload: State,
        }
        let c: Checkpoint = parse(&read(path)?, "nrl.adaptation-checkpoint/0")?;
        if c.schema != "nrl.adaptation-checkpoint/0" || c.payload_sha256 != id(&c.payload)? {
            return Err("adaptation checkpoint hash mismatch".into());
        }
        c.payload.validate(teacher)?;
        Ok(c.payload)
    }
    pub fn export(&self, teacher: &Decoder) -> Result<Decoder> {
        self.validate(teacher)?;
        let mut source = self.config.source.clone();
        let hash = id(self)?;
        source.identifier = format!("{}#{}", source.identifier, hash);
        source.sha256 = Some(hash);
        self.model(teacher, &self.best)?
            .with_patches(self.best.clone(), &[self.corpus.source.clone(), source])
    }
}
pub fn compare(
    teacher: &Decoder,
    initial: &Decoder,
    corpus: Corpus,
    config: Config,
    path: &Path,
) -> Result<serde_json::Value> {
    config.validate()?;
    if config.optimizer.algorithm != "adamw" {
        return Err("comparison config must name AdamW".into());
    }
    fs::create_dir(path).map_err(|e| e.to_string())?;
    let mut adam = State::start(teacher, initial, corpus.clone(), config.clone())?;
    let mut control = config.clone();
    control.optimizer.algorithm = "sgd".into();
    control.optimizer.beta1 = 0.0;
    control.optimizer.beta2 = 0.0;
    control.optimizer.weight_decay = 0.0;
    let mut sgd = State::start(teacher, initial, corpus.clone(), control)?;
    adam.advance(teacher, config.optimizer.max_steps)?;
    sgd.advance(teacher, config.optimizer.max_steps)?;
    adam.save(teacher, &path.join("adamw-checkpoint.json"))?;
    sgd.save(teacher, &path.join("sgd-checkpoint.json"))?;
    let a = adam.export(teacher)?;
    let s = sgd.export(teacher)?;
    a.save(&path.join("adamw-model"))?;
    s.save(&path.join("sgd-model"))?;
    let mut reports = BTreeMap::new();
    for (name, m) in [
        ("dense", teacher),
        ("initial", initial),
        ("adamw", &a),
        ("sgd", &s),
    ] {
        for split in ["validation", "test"] {
            let r = evaluate::evaluate_language(teacher, m, &corpus, split, None)?;
            r.save(&path.join(format!("{name}-{split}.json")))?;
            reports.insert(format!("{name}-{split}"), r);
        }
    }
    let dense = &reports["dense-validation"];
    let candidate = &reports["adamw-validation"];
    let ratio = candidate.model_information["package_bytes"]
        .as_f64()
        .ok_or("missing package bytes")?
        / dense.model_information["package_bytes"]
            .as_f64()
            .ok_or("missing baseline bytes")?;
    let accepted = candidate.quality.value
        <= dense.quality.value + config.max_validation_nll_increase
        && ratio <= config.max_package_ratio;
    let summary = serde_json::json!({"schema":"nrl.adaptation-comparison/0","corpus_id":corpus.data_id()?,"config":config,"adamw_selected_step":adam.best_step,"sgd_selected_step":sgd.best_step,"adamw_validation_history":adam.validation_nll,"sgd_validation_history":sgd.validation_nll,"reports":reports,"candidate_package_ratio":ratio,"recommendation":if accepted{"adamw-candidate"}else{"dense"},"test_used_for_selection":false,"training_scope":"local replaced FFNs, refreshed teacher targets on student inputs; no attention/embedding/whole-decoder backpropagation","control":"same rate, steps, initialization, data order, local MSE and validation selection; SGD without moments or decay","native_execution":false});
    save(&path.join("summary.json"), &summary)?;
    Ok(summary)
}
