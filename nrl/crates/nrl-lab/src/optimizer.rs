//! Bounded host optimizers with a complete, parameter-bound continuation state.
use crate::{
    model::{Model, ObjectData, ObjectGradients},
    research, Artifact, OwnedGradients, Result,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub type Tensors = BTreeMap<String, Vec<f64>>;
const MAX_VALUES: usize = 262144;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub algorithm: String,
    pub learning_rate: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub epsilon: f64,
    pub weight_decay: f64,
    pub max_steps: usize,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        research::finite(&[
            self.learning_rate,
            self.beta1,
            self.beta2,
            self.epsilon,
            self.weight_decay,
        ])?;
        if self.schema != "nrl.optimizer/0"
            || !matches!(self.algorithm.as_str(), "adamw" | "sgd")
            || !(0.0..=1.0).contains(&self.learning_rate)
            || self.learning_rate == 0.0
            || !(0.0..1.0).contains(&self.beta1)
            || !(0.0..1.0).contains(&self.beta2)
            || self.epsilon <= 0.0
            || !(0.0..=1.0).contains(&self.weight_decay)
            || self.max_steps == 0
            || self.max_steps > 256
            || (self.algorithm == "sgd"
                && (self.beta1 != 0.0 || self.beta2 != 0.0 || self.weight_decay != 0.0))
        {
            return Err("unsupported optimizer configuration/rate/budget".into());
        }
        Ok(())
    }
}
fn inventory(p: &Tensors) -> Result<BTreeMap<String, usize>> {
    if p.is_empty() || p.len() > 512 || p.keys().any(|k| !research::name(k)) {
        return Err("optimizer tensor inventory/name limit".into());
    }
    let count = p
        .values()
        .try_fold(0usize, |n, v| n.checked_add(v.len()))
        .ok_or("optimizer size overflow")?;
    if count == 0 || count > MAX_VALUES {
        return Err("optimizer value budget".into());
    }
    for v in p.values() {
        research::finite(v)?;
    }
    Ok(p.iter().map(|(k, v)| (k.clone(), v.len())).collect())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema: String,
    pub config: Config,
    pub step: usize,
    pub parameter_id: String,
    pub first: Tensors,
    pub second: Tensors,
}
impl State {
    pub fn start(config: Config, parameters: &Tensors) -> Result<Self> {
        config.validate()?;
        inventory(parameters)?;
        let zero = parameters
            .iter()
            .map(|(k, v)| (k.clone(), vec![0.0; v.len()]))
            .collect();
        Ok(Self {
            schema: "nrl.optimizer-state/0".into(),
            config,
            step: 0,
            parameter_id: research::id(parameters)?,
            first: zero,
            second: parameters
                .iter()
                .map(|(k, v)| (k.clone(), vec![0.0; v.len()]))
                .collect(),
        })
    }
    pub fn validate(&self, parameters: &Tensors) -> Result<()> {
        self.config.validate()?;
        let shapes = inventory(parameters)?;
        if self.schema != "nrl.optimizer-state/0"
            || self.step > self.config.max_steps
            || self.parameter_id != research::id(parameters)?
            || inventory(&self.first)? != shapes
            || inventory(&self.second)? != shapes
            || self.second.values().flatten().any(|v| *v < 0.0)
            || ((self.step == 0 || self.config.algorithm == "sgd")
                && self
                    .first
                    .values()
                    .chain(self.second.values())
                    .flatten()
                    .any(|v| *v != 0.0))
        {
            return Err("optimizer state/parameter/moment binding mismatch".into());
        }
        Ok(())
    }
    pub fn update(&mut self, parameters: &mut Tensors, gradients: &Tensors) -> Result<()> {
        self.validate(parameters)?;
        if self.step >= self.config.max_steps || inventory(gradients)? != inventory(parameters)? {
            return Err("optimizer step/gradient inventory limit".into());
        }
        let mut next = self.clone();
        let mut result = parameters.clone();
        next.step += 1;
        let c = &self.config;
        let correction1 = 1.0 - c.beta1.powi(next.step as i32);
        let correction2 = 1.0 - c.beta2.powi(next.step as i32);
        for (key, p) in &mut result {
            for (i, w) in p.iter_mut().enumerate() {
                let g = gradients[key][i];
                if c.algorithm == "sgd" {
                    *w -= c.learning_rate * g;
                } else {
                    let m = &mut next.first.get_mut(key).unwrap()[i];
                    let v = &mut next.second.get_mut(key).unwrap()[i];
                    *m = c.beta1 * *m + (1.0 - c.beta1) * g;
                    *v = c.beta2 * *v + (1.0 - c.beta2) * g * g;
                    *w *= 1.0 - c.learning_rate * c.weight_decay;
                    *w -= c.learning_rate * (*m / correction1)
                        / ((*v / correction2).sqrt() + c.epsilon);
                }
            }
        }
        next.parameter_id = research::id(&result)?;
        next.validate(&result)?;
        *parameters = result;
        *self = next;
        Ok(())
    }
}
pub(crate) fn block_parameters(a: &Artifact) -> Result<Vec<Vec<f64>>> {
    a.validate()?;
    let mut t = vec![
        a.gate.clone(),
        a.up.clone(),
        a.down.clone(),
        a.spline
            .as_ref()
            .map_or_else(Vec::new, |s| s.values.clone()),
    ];
    if let Some(f) = &a.factors {
        t.extend(f.tensors().iter().map(|s| s.to_vec()));
    }
    if let Some(b) = &a.bias {
        t.extend(b.tensors().iter().map(|s| s.to_vec()));
    }
    Ok(t)
}
pub(crate) fn replace_block(a: &Artifact, tensors: &[Vec<f64>]) -> Result<Artifact> {
    if tensors.iter().map(Vec::len).collect::<Vec<_>>()
        != block_parameters(a)?
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>()
    {
        return Err("block tensor geometry mismatch".into());
    }
    let mut next = a.clone();
    let mut values = tensors.iter();
    next.gate = values.next().unwrap().clone();
    next.up = values.next().unwrap().clone();
    next.down = values.next().unwrap().clone();
    let spline = values.next().unwrap();
    if let Some(s) = &mut next.spline {
        s.values = spline.clone();
    }
    if let Some(f) = &mut next.factors {
        for t in f.tensors_mut() {
            t.copy_from_slice(values.next().unwrap());
        }
    }
    if let Some(b) = &mut next.bias {
        for t in b.tensors_mut() {
            t.copy_from_slice(values.next().unwrap());
        }
    }
    next.validate()?;
    Ok(next)
}
pub(crate) fn insert(t: &mut Tensors, prefix: &str, values: Vec<Vec<f64>>) {
    for (i, v) in values.into_iter().enumerate() {
        t.insert(format!("{prefix}.{i}"), v);
    }
}
pub(crate) fn block_gradients(g: &OwnedGradients) -> Vec<Vec<f64>> {
    g.tensors().iter().map(|v| v.to_vec()).collect()
}
pub(crate) fn take(t: &Tensors, prefix: &str, count: usize) -> Result<Vec<Vec<f64>>> {
    (0..count)
        .map(|i| {
            t.get(&format!("{prefix}.{i}"))
                .cloned()
                .ok_or_else(|| "missing parameter tensor".into())
        })
        .collect()
}
pub(crate) fn model_parameters(m: &Model) -> Result<Tensors> {
    let mut t = Tensors::new();
    for key in m.manifest().objects.keys() {
        let values = match m.object(key).ok_or("missing model object")? {
            ObjectData::Affine(a) => vec![a.weights.clone(), a.bias.clone()],
            ObjectData::Gated(a) => block_parameters(a)?,
        };
        insert(&mut t, &format!("p.{key}"), values);
    }
    inventory(&t)?;
    Ok(t)
}
pub(crate) fn model_gradients(g: &BTreeMap<String, ObjectGradients>) -> Tensors {
    let mut t = Tensors::new();
    for (key, g) in g {
        insert(
            &mut t,
            &format!("p.{key}"),
            g.tensors().iter().map(|v| v.to_vec()).collect(),
        );
    }
    t
}
pub(crate) fn replace_model(m: &Model, t: &Tensors) -> Result<Model> {
    let mut objects = BTreeMap::new();
    for key in m.manifest().objects.keys() {
        let prefix = format!("p.{key}");
        let data = match m.object(key).ok_or("missing model object")? {
            ObjectData::Affine(a) => {
                let mut a = a.clone();
                let v = take(t, &prefix, 2)?;
                a.weights = v[0].clone();
                a.bias = v[1].clone();
                ObjectData::Affine(a)
            }
            ObjectData::Gated(a) => ObjectData::Gated(replace_block(
                a,
                &take(t, &prefix, block_parameters(a)?.len())?,
            )?),
        };
        objects.insert(key.clone(), data);
    }
    m.with_parameters(objects, &[])
}
