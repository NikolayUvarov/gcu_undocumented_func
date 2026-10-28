use super::*;
use crate::OwnedGradients;
use nrl_repr::affine::AffineGradients;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ObjectGradients {
    Affine { weights: Vec<f64>, bias: Vec<f64> },
    Gated(OwnedGradients),
}
impl ObjectGradients {
    pub fn tensors(&self) -> Vec<&[f64]> {
        match self {
            Self::Affine { weights, bias } => vec![weights, bias],
            Self::Gated(g) => g.tensors(),
        }
    }
}
#[derive(Debug)]
pub struct Backward {
    pub output: Vec<f64>,
    pub input_gradient: Vec<f64>,
    pub gradients: BTreeMap<String, ObjectGradients>,
}
impl ObjectData {
    fn zero_gradients(&self) -> ObjectGradients {
        match self {
            Self::Affine(a) => ObjectGradients::Affine {
                weights: vec![0.0; a.weights.len()],
                bias: vec![0.0; a.bias.len()],
            },
            Self::Gated(a) => ObjectGradients::Gated(OwnedGradients::zeros(a)),
        }
    }
    fn backward(&self, x: &[f64], dy: &[f64], gradients: &mut ObjectGradients) -> Result<Vec<f64>> {
        let summary = self.summary()?;
        let mut dx = vec![0.0; x.len()];
        if x.is_empty()
            || x.len() % summary.input_width != 0
            || dy.len() != x.len() / summary.input_width * summary.output_width
        {
            return Err("object backward shape mismatch".into());
        }
        match (self, gradients) {
            (Self::Affine(a), ObjectGradients::Affine { weights, bias }) => {
                let op = a.view()?;
                for ((x, dy), dx) in x
                    .chunks_exact(a.shape[0])
                    .zip(dy.chunks_exact(a.shape[1]))
                    .zip(dx.chunks_exact_mut(a.shape[0]))
                {
                    numeric(op.backward(x, dy, dx, &mut AffineGradients { weights, bias }))?;
                }
            }
            (Self::Gated(a), ObjectGradients::Gated(g)) => {
                let block = a.block()?;
                let mut scratch = vec![0.0; block.scratch_len()];
                for ((x, dy), dx) in x
                    .chunks_exact(a.shape[0])
                    .zip(dy.chunks_exact(a.shape[2]))
                    .zip(dx.chunks_exact_mut(a.shape[0]))
                {
                    numeric(block.backward(x, dy, dx, g, &mut scratch))?;
                }
            }
            _ => return Err("object gradient kind mismatch".into()),
        }
        numeric(nrl_core::finite(&dx))?;
        Ok(dx)
    }
    fn step(&self, gradient: &ObjectGradients, rate: f64) -> Result<Self> {
        match (self, gradient) {
            (Self::Gated(a), ObjectGradients::Gated(g)) => Ok(Self::Gated(a.step(g, rate)?)),
            (Self::Affine(a), ObjectGradients::Affine { weights, bias }) => {
                let mut next = a.clone();
                for (values, gradients) in [(&mut next.weights, weights), (&mut next.bias, bias)] {
                    if values.len() != gradients.len() {
                        return Err("affine gradient shape mismatch".into());
                    }
                    numeric(nrl_core::finite(gradients))?;
                    for (w, g) in values.iter_mut().zip(gradients) {
                        *w -= rate * g;
                    }
                }
                next.view()?;
                Ok(Self::Affine(next))
            }
            _ => Err("object gradient kind mismatch".into()),
        }
    }
}
fn accumulate(target: &mut [f64], source: &[f64]) -> Result<()> {
    if target.len() != source.len() {
        return Err("graph adjoint shape mismatch".into());
    }
    for (a, b) in target.iter_mut().zip(source) {
        *a += b;
    }
    numeric(nrl_core::finite(target))
}
impl Model {
    pub fn backward(
        &self,
        input: &[f64],
        dy: &[f64],
        max_training_values: usize,
    ) -> Result<Backward> {
        let c = check_graph(
            &self.manifest.descriptor,
            &self.manifest.execution,
            &self.objects,
        )?;
        let graph = &self.manifest.descriptor.configuration;
        if input.is_empty() || input.len() % graph.input_width != 0 {
            return Err("invalid model input shape".into());
        }
        let rows = input.len() / graph.input_width;
        let values = rows
            .checked_mul(c.values_per_row)
            .and_then(|v| v.checked_mul(2))
            .and_then(|v| v.checked_add(c.trainable_values));
        if max_training_values == 0
            || max_training_values > MAX_ACTIVATION_VALUES
            || values.is_none_or(|v| v > max_training_values)
        {
            return Err("model training value budget exceeded".into());
        }
        if dy.len() != rows * c.widths[&graph.output] {
            return Err("model output gradient shape mismatch".into());
        }
        numeric(nrl_core::finite(dy))?;
        let values = self.forward_values(input)?;
        let mut adjoints: BTreeMap<_, _> = values
            .iter()
            .map(|(k, v)| (k.clone(), vec![0.0; v.len()]))
            .collect();
        adjoints.get_mut(&graph.output).unwrap().copy_from_slice(dy);
        let mut gradients: BTreeMap<_, _> = self
            .objects
            .iter()
            .map(|(k, o)| (k.clone(), o.data.zero_gradients()))
            .collect();
        for node in graph.nodes.iter().rev() {
            let derivative = adjoints.remove(&node.id).ok_or("missing graph adjoint")?;
            match &node.op {
                Operation::Affine { input, object } | Operation::Gated { input, object } => {
                    let dx = self.objects[object].data.backward(
                        &values[input],
                        &derivative,
                        gradients.get_mut(object).unwrap(),
                    )?;
                    accumulate(adjoints.get_mut(input).unwrap(), &dx)?;
                }
                Operation::Silu { input } | Operation::Relu { input } => {
                    let dx: Vec<_> = values[input]
                        .iter()
                        .zip(&derivative)
                        .map(|(x, dy)| {
                            let d = if matches!(node.op, Operation::Silu { .. }) {
                                nrl_repr::silu(*x).1
                            } else if *x > 0.0 {
                                1.0
                            } else {
                                0.0
                            };
                            d * dy
                        })
                        .collect();
                    accumulate(adjoints.get_mut(input).unwrap(), &dx)?;
                }
                Operation::Add { left, right } => {
                    accumulate(adjoints.get_mut(left).unwrap(), &derivative)?;
                    accumulate(adjoints.get_mut(right).unwrap(), &derivative)?;
                }
            }
        }
        for g in gradients.values() {
            for t in g.tensors() {
                numeric(nrl_core::finite(t))?;
            }
        }
        Ok(Backward {
            output: values[&graph.output].clone(),
            input_gradient: adjoints.remove("input").ok_or("missing input adjoint")?,
            gradients,
        })
    }
    // Return replacement data; packaging and its provenance are separate operations.
    pub fn sgd_parameters(
        &self,
        gradients: &BTreeMap<String, ObjectGradients>,
        rate: f64,
    ) -> Result<BTreeMap<String, ObjectData>> {
        if !rate.is_finite() || rate <= 0.0 {
            return Err("learning rate must be positive and finite".into());
        }
        if gradients.keys().ne(self.objects.keys()) {
            return Err("model gradient inventory mismatch".into());
        }
        self.objects
            .iter()
            .map(|(k, o)| Ok((k.clone(), o.data.step(&gradients[k], rate)?)))
            .collect()
    }
    pub(crate) fn with_parameters(
        &self,
        parameters: BTreeMap<String, ObjectData>,
        sources: &[Source],
    ) -> Result<Self> {
        if parameters.keys().ne(self.objects.keys()) {
            return Err("training object inventory mismatch".into());
        }
        let mut objects = BTreeMap::new();
        for (key, data) in parameters {
            let next = Object::new(data)?;
            if next.summary != self.objects[&key].summary {
                return Err("training changes parameter geometry".into());
            }
            objects.insert(key, Arc::new(next));
        }
        let mut manifest = self.standalone()?.manifest;
        for source in sources {
            if !manifest.sources.contains(source) {
                manifest.sources.push(source.clone());
            }
        }
        manifest.objects = objects
            .iter()
            .enumerate()
            .map(|(i, (k, o))| (k.clone(), record(i, o)))
            .collect();
        Self::assemble(manifest, objects, 0)
    }
    pub(crate) fn with_sources(&self, sources: &[Source]) -> Result<Self> {
        let mut result = self.standalone()?;
        for source in sources {
            if !result.manifest.sources.contains(source) {
                result.manifest.sources.push(source.clone());
            }
        }
        Self::assemble(result.manifest, result.objects, 0)
    }
}
