use crate::{import_json, storage, Artifact, Result, MAX_ARTIFACT_BYTES, MAX_ARTIFACT_VALUES};
use nrl_repr::affine::Affine;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub const MAX_MODEL_BYTES: usize = 16 << 20;
pub const MAX_MANIFEST_BYTES: usize = 1 << 20;
pub const MAX_NODES: usize = 64;
pub const MAX_OBJECTS: usize = 64;
pub const MAX_NUMERIC_VALUES: usize = 262144;
pub const MAX_ACTIVATION_VALUES: usize = 1 << 20;
pub const MAX_HISTORY: usize = 128;

fn numeric<T>(r: nrl_core::Result<T>) -> Result<T> {
    r.map_err(|e| format!("model numerical contract: {e:?}"))
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn count_add(a: usize, b: usize, limit: usize) -> Result<usize> {
    a.checked_add(b)
        .filter(|n| *n <= limit)
        .ok_or("model resource limit exceeded".into())
}
fn require_schema(value: &serde_json::Value, expected: &str) -> Result<()> {
    let schema = value["schema"]
        .as_str()
        .ok_or("missing model format schema")?;
    if schema != expected {
        return Err(format!("unsupported schema: {schema}; expected {expected}"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffineData {
    pub schema: String,
    pub shape: [usize; 2],
    pub weights: Vec<f64>,
    pub bias: Vec<f64>,
}
impl AffineData {
    pub fn view(&self) -> Result<Affine<'_>> {
        if self.schema != "nrl.affine/0" {
            return Err(format!("unsupported affine schema: {}", self.schema));
        }
        if self
            .weights
            .len()
            .checked_add(self.bias.len())
            .is_none_or(|n| n > MAX_ARTIFACT_VALUES)
        {
            return Err("affine value limit exceeded".into());
        }
        numeric(Affine::new(
            self.shape[0],
            self.shape[1],
            &self.weights,
            &self.bias,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ObjectData {
    Affine(AffineData),
    Gated(Artifact),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub kind: String,
    pub schema: String,
    pub input_width: usize,
    pub output_width: usize,
    pub representation: String,
    pub channels: Option<usize>,
    pub rank: Option<usize>,
    pub knots: Option<usize>,
    pub numeric_values: usize,
    pub trainable_values: usize,
    pub bias_values: usize,
}
impl ObjectData {
    pub fn summary(&self) -> Result<Summary> {
        match self {
            Self::Affine(a) => {
                a.view()?;
                let values = a.weights.len() + a.bias.len();
                Ok(Summary {
                    kind: "affine".into(),
                    schema: a.schema.clone(),
                    input_width: a.shape[0],
                    output_width: a.shape[1],
                    representation: "dense".into(),
                    channels: None,
                    rank: None,
                    knots: None,
                    numeric_values: values,
                    trainable_values: values,
                    bias_values: a.bias.len(),
                })
            }
            Self::Gated(a) => {
                a.validate()?;
                let mut trainable = a.gate.len() + a.up.len() + a.down.len();
                if let Some(f) = &a.factors {
                    trainable += f.tensors().iter().map(|v| v.len()).sum::<usize>();
                }
                let bias = a
                    .bias
                    .as_ref()
                    .map_or(0, |b| b.tensors().iter().map(|v| v.len()).sum());
                trainable += bias + a.spline.as_ref().map_or(0, |s| s.values.len());
                let numeric_values =
                    trainable + a.spline.as_ref().map_or(0, |s| s.low.len() + s.high.len());
                Ok(Summary {
                    kind: "gated".into(),
                    schema: a.schema.clone(),
                    input_width: a.shape[0],
                    output_width: a.shape[2],
                    representation: a.representation.clone(),
                    channels: Some(a.shape[1]),
                    rank: a.factors.as_ref().map(|f| f.rank),
                    knots: a.spline.as_ref().map(|s| s.knots),
                    numeric_values,
                    trainable_values: trainable,
                    bias_values: bias,
                })
            }
        }
    }
    fn forward(&self, input: &[f64]) -> Result<Vec<f64>> {
        match self {
            Self::Gated(a) => a.forward(input),
            Self::Affine(a) => {
                let op = a.view()?;
                if input.is_empty() || input.len() % a.shape[0] != 0 {
                    return Err("invalid affine input batch".into());
                }
                let rows = input.len() / a.shape[0];
                let count = numeric(nrl_core::elements(rows, a.shape[1]))?;
                let mut output = vec![0.0; count];
                for (x, y) in input
                    .chunks_exact(a.shape[0])
                    .zip(output.chunks_exact_mut(a.shape[1]))
                {
                    numeric(op.forward(x, y))?;
                }
                Ok(output)
            }
        }
    }
}
fn decode_object(value: serde_json::Value) -> Result<ObjectData> {
    match value["kind"].as_str() {
        Some("gated") => require_schema(&value["data"], "nrl.block/5")?,
        Some("affine") => require_schema(&value["data"], "nrl.affine/0")?,
        other => return Err(format!("unsupported model object kind: {other:?}")),
    }
    let data: ObjectData = serde_json::from_value(value).map_err(|e| e.to_string())?;
    data.summary()?;
    Ok(data)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operator", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    Affine { input: String, object: String },
    Gated { input: String, object: String },
    Silu { input: String },
    Relu { input: String },
    Add { left: String, right: String },
}
impl Operation {
    fn inputs(&self) -> Vec<&str> {
        match self {
            Self::Affine { input, .. }
            | Self::Gated { input, .. }
            | Self::Silu { input }
            | Self::Relu { input } => vec![input],
            Self::Add { left, right } => vec![left, right],
        }
    }
    fn object(&self) -> Option<&str> {
        match self {
            Self::Affine { object, .. } | Self::Gated { object, .. } => Some(object),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub op: Operation,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub schema: String,
    pub input_width: usize,
    pub nodes: Vec<Node>,
    pub output: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Tie {
    pub object: String,
    pub nodes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub family: String,
    pub adapter: String,
    pub adapter_version: usize,
    pub configuration: Graph,
    pub preprocessor: String,
    pub tasks: Vec<String>,
    pub state: String,
    pub tied_objects: Vec<Tie>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub schema: String,
    pub backend: String,
    pub dtype: String,
    pub layout: String,
    pub max_batch_rows: usize,
    pub max_activation_values: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub identifier: String,
    pub sha256: Option<String>,
    pub licence: String,
    pub notices: Vec<String>,
}
fn validate_sources(sources: &[Source]) -> Result<()> {
    if sources.is_empty() || sources.len() > 32 {
        return Err("model source count limit".into());
    }
    for source in sources {
        if source.identifier.is_empty()
            || source.identifier.len() > 2048
            || source.licence.trim().is_empty()
            || source.licence.len() > 4096
            || source
                .sha256
                .as_ref()
                .is_some_and(|s| !storage::valid_hash(s))
            || source.notices.len() > 16
            || source.notices.iter().any(|n| n.len() > 16384)
        {
            return Err("invalid model source, licence or notice".into());
        }
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub schema: String,
    pub descriptor: Descriptor,
    pub execution: Execution,
    pub sources: Vec<Source>,
    pub objects: BTreeMap<String, ObjectData>,
}
impl Definition {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut value = import_json::parse(bytes, MAX_MODEL_BYTES)?;
        require_schema(&value, "nrl.model-definition/0")?;
        let objects = value["objects"]
            .as_object()
            .ok_or("model definition requires objects")?;
        if objects.is_empty() || objects.len() > MAX_OBJECTS {
            return Err("model object count limit".into());
        }
        for object in objects.values() {
            decode_object(object.clone())?;
        }
        let definition = serde_json::from_value(value.take()).map_err(|e| e.to_string())?;
        Ok(definition)
    }
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(&storage::read_regular(path, MAX_MODEL_BYTES)?)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replacements {
    pub schema: String,
    pub source: Source,
    pub reason: String,
    pub objects: BTreeMap<String, ObjectData>,
}
impl Replacements {
    pub fn load(path: &Path) -> Result<Self> {
        let value = import_json::parse(
            &storage::read_regular(path, MAX_MODEL_BYTES)?,
            MAX_MODEL_BYTES,
        )?;
        require_schema(&value, "nrl.replacements/0")?;
        let objects = value["objects"]
            .as_object()
            .ok_or("replacement object map required")?;
        if objects.is_empty() || objects.len() > MAX_OBJECTS {
            return Err("replacement count limit".into());
        }
        for object in objects.values() {
            decode_object(object.clone())?;
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectRecord {
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
    pub parameters: Summary,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub model_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub object: String,
    pub operation: String,
    pub base_model_id: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub parameters: Summary,
    pub reason: String,
    pub source_identifier: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub kind: String,
    pub descriptor: Descriptor,
    pub execution: Execution,
    pub base: Option<Dependency>,
    pub objects: BTreeMap<String, ObjectRecord>,
    pub sources: Vec<Source>,
    pub history: Vec<Change>,
    pub changes: Vec<Change>,
}
struct Object {
    data: ObjectData,
    bytes: Vec<u8>,
    summary: Summary,
}
impl Object {
    fn new(data: ObjectData) -> Result<Self> {
        let summary = data.summary()?;
        let bytes = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("model object byte limit".into());
        }
        Ok(Self {
            data,
            bytes,
            summary,
        })
    }
}
pub struct Model {
    manifest: Manifest,
    model_id: String,
    objects: BTreeMap<String, Arc<Object>>,
    package_bytes: usize,
    dependency_bytes: usize,
}
struct CheckedGraph {
    widths: BTreeMap<String, usize>,
    values_per_row: usize,
    numeric_values: usize,
    trainable_values: usize,
}
fn check_graph(
    d: &Descriptor,
    e: &Execution,
    objects: &BTreeMap<String, Arc<Object>>,
) -> Result<CheckedGraph> {
    if d.family != "feed-forward"
        || d.adapter != "nrl.feed-forward"
        || d.adapter_version != 0
        || d.preprocessor != "identity-f64/0"
        || d.state != "stateless"
    {
        return Err("unsupported model family, adapter, preprocessor or state".into());
    }
    if d.tasks.is_empty()
        || d.tasks.len() > 2
        || d.tasks.iter().any(|t| {
            !matches!(
                t.as_str(),
                "regression-vector/0" | "classification-logits/0"
            )
        })
        || d.tasks.iter().collect::<BTreeSet<_>>().len() != d.tasks.len()
    {
        return Err("unsupported or duplicate model task".into());
    }
    if e.schema != "nrl.execution/0" {
        return Err(format!("unsupported execution schema: {}", e.schema));
    }
    if e.schema != "nrl.execution/0"
        || e.backend != "cpu-scalar"
        || e.dtype != "f64"
        || e.layout != "row-major"
        || e.max_batch_rows == 0
        || e.max_batch_rows > 4096
        || e.max_activation_values == 0
        || e.max_activation_values > MAX_ACTIVATION_VALUES
    {
        return Err("unsupported model execution profile".into());
    }
    let g = &d.configuration;
    if g.schema != "nrl.graph/0" {
        return Err(format!("unsupported graph schema: {}", g.schema));
    }
    if g.schema != "nrl.graph/0"
        || g.input_width == 0
        || g.input_width > 4096
        || g.nodes.is_empty()
        || g.nodes.len() > MAX_NODES
        || objects.is_empty()
        || objects.len() > MAX_OBJECTS
    {
        return Err("invalid graph schema, dimensions or count".into());
    }
    let mut numeric_values = 0;
    let mut trainable_values = 0;
    for (key, object) in objects {
        if !id(key) {
            return Err("invalid object ID".into());
        }
        numeric_values = count_add(
            numeric_values,
            object.summary.numeric_values,
            MAX_NUMERIC_VALUES,
        )?;
        trainable_values = count_add(
            trainable_values,
            object.summary.trainable_values,
            MAX_NUMERIC_VALUES,
        )?;
    }
    let mut widths = BTreeMap::from([("input".to_owned(), g.input_width)]);
    let mut aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut values_per_row = g.input_width;
    for node in &g.nodes {
        if !id(&node.id) || widths.contains_key(&node.id) {
            return Err("invalid or duplicate graph node ID".into());
        }
        let inputs = node.op.inputs();
        let mut input_widths = Vec::new();
        for input in inputs {
            input_widths.push(
                *widths.get(input).ok_or_else(|| {
                    format!("missing, forward or cyclic graph reference: {input}")
                })?,
            );
        }
        let width = if let Some(key) = node.op.object() {
            let object = objects
                .get(key)
                .ok_or_else(|| format!("missing model object: {key}"))?;
            let expected = match node.op {
                Operation::Affine { .. } => "affine",
                _ => "gated",
            };
            if object.summary.kind != expected || object.summary.input_width != input_widths[0] {
                return Err("graph operator kind or input shape mismatch".into());
            }
            aliases
                .entry(key.to_string())
                .or_default()
                .push(node.id.clone());
            object.summary.output_width
        } else {
            if input_widths.len() == 2 && input_widths[0] != input_widths[1] {
                return Err("graph add width mismatch".into());
            }
            input_widths[0]
        };
        if width > 4096 {
            return Err("graph width limit".into());
        }
        values_per_row = count_add(values_per_row, width, e.max_activation_values)?;
        widths.insert(node.id.clone(), width);
    }
    if aliases.len() != objects.len() {
        return Err("unused model object".into());
    }
    let ties: Vec<Tie> = aliases
        .into_iter()
        .filter(|(_, nodes)| nodes.len() > 1)
        .map(|(object, nodes)| Tie { object, nodes })
        .collect();
    if ties != d.tied_objects {
        return Err("declared tied-object aliases do not match the graph".into());
    }
    if g.output == "input" || !widths.contains_key(&g.output) {
        return Err("missing graph output".into());
    }
    let mut needed = BTreeSet::from([g.output.as_str()]);
    for node in g.nodes.iter().rev() {
        if needed.contains(node.id.as_str()) {
            needed.extend(node.op.inputs());
        }
    }
    if g.nodes.iter().any(|n| !needed.contains(n.id.as_str())) {
        return Err("unreachable graph node".into());
    }
    Ok(CheckedGraph {
        widths,
        values_per_row,
        numeric_values,
        trainable_values,
    })
}
fn check_history(records: &[Change]) -> Result<()> {
    if records.len() > MAX_HISTORY {
        return Err("model history limit".into());
    }
    for c in records {
        if !id(&c.object)
            || c.operation != "manual-parameter-replacement/0"
            || !storage::valid_hash(&c.base_model_id)
            || !storage::valid_hash(&c.before_sha256)
            || !storage::valid_hash(&c.after_sha256)
            || c.reason.trim().is_empty()
            || c.reason.len() > 4096
            || c.source_identifier.is_empty()
            || c.source_identifier.len() > 2048
        {
            return Err("invalid model change history".into());
        }
    }
    Ok(())
}
fn record(index: usize, object: &Object) -> ObjectRecord {
    ObjectRecord {
        file: format!("object-{index:04}.json"),
        sha256: storage::hash(&object.bytes),
        bytes: object.bytes.len(),
        parameters: object.summary.clone(),
    }
}
fn identity(manifest: &Manifest) -> Result<String> {
    let value = serde_json::to_value(manifest).map_err(|e| e.to_string())?;
    Ok(storage::hash(
        import_json::canonical_compact(&value).as_bytes(),
    ))
}

impl Model {
    pub fn from_definition(definition: Definition) -> Result<Self> {
        if definition.schema != "nrl.model-definition/0" {
            return Err(format!(
                "unsupported model definition schema: {}",
                definition.schema
            ));
        }
        if definition.objects.is_empty() || definition.objects.len() > MAX_OBJECTS {
            return Err("model object count limit".into());
        }
        validate_sources(&definition.sources)?;
        let objects: BTreeMap<String, Arc<Object>> = definition
            .objects
            .into_iter()
            .map(|(k, v)| Ok((k, Arc::new(Object::new(v)?))))
            .collect::<Result<_>>()?;
        check_graph(&definition.descriptor, &definition.execution, &objects)?;
        let entries = objects
            .iter()
            .enumerate()
            .map(|(i, (k, v))| (k.clone(), record(i, v)))
            .collect();
        let manifest = Manifest {
            schema: "nrl.model/0".into(),
            kind: "standalone".into(),
            descriptor: definition.descriptor,
            execution: definition.execution,
            base: None,
            objects: entries,
            sources: definition.sources,
            history: vec![],
            changes: vec![],
        };
        Self::assemble(manifest, objects, 0)
    }
    fn assemble(
        manifest: Manifest,
        objects: BTreeMap<String, Arc<Object>>,
        dependency_bytes: usize,
    ) -> Result<Self> {
        check_graph(&manifest.descriptor, &manifest.execution, &objects)?;
        validate_sources(&manifest.sources)?;
        check_history(&manifest.history)?;
        check_history(&manifest.changes)?;
        if manifest.history.len() + manifest.changes.len() > MAX_HISTORY {
            return Err("model history limit".into());
        }
        let mut package_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .len();
        if package_bytes > MAX_MANIFEST_BYTES {
            return Err("model manifest byte limit".into());
        }
        for entry in manifest.objects.values() {
            package_bytes = count_add(package_bytes, entry.bytes, MAX_MODEL_BYTES)?;
        }
        let model_id = identity(&manifest)?;
        Ok(Self {
            manifest,
            model_id,
            objects,
            package_bytes,
            dependency_bytes,
        })
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    pub fn object(&self, key: &str) -> Option<&ObjectData> {
        self.objects.get(key).map(|o| &o.data)
    }
    pub fn forward(&self, input: &[f64]) -> Result<Vec<f64>> {
        let checked = check_graph(
            &self.manifest.descriptor,
            &self.manifest.execution,
            &self.objects,
        )?;
        let graph = &self.manifest.descriptor.configuration;
        if input.is_empty() || input.len() % graph.input_width != 0 {
            return Err("invalid model input shape".into());
        }
        let rows = input.len() / graph.input_width;
        if rows > self.manifest.execution.max_batch_rows
            || rows
                .checked_mul(checked.values_per_row)
                .is_none_or(|n| n > self.manifest.execution.max_activation_values)
        {
            return Err("model batch or activation budget exceeded".into());
        }
        numeric(nrl_core::finite(input))?;
        let mut values = BTreeMap::from([("input".to_owned(), input.to_vec())]);
        for node in &graph.nodes {
            let output = match &node.op {
                Operation::Affine { input, object } | Operation::Gated { input, object } => {
                    self.objects[object].data.forward(&values[input])?
                }
                Operation::Silu { input } => {
                    values[input].iter().map(|v| nrl_repr::silu(*v).0).collect()
                }
                Operation::Relu { input } => values[input].iter().map(|v| v.max(0.0)).collect(),
                Operation::Add { left, right } => values[left]
                    .iter()
                    .zip(&values[right])
                    .map(|(a, b)| a + b)
                    .collect(),
            };
            numeric(nrl_core::finite(&output))?;
            values.insert(node.id.clone(), output);
        }
        values
            .remove(&graph.output)
            .ok_or("missing computed graph output".into())
    }
    pub fn overlay(&self, replacements: Replacements) -> Result<Self> {
        if self.manifest.kind != "standalone" {
            return Err("overlay base must be a standalone model; chains are unsupported".into());
        }
        if replacements.schema != "nrl.replacements/0" {
            return Err(format!(
                "unsupported replacement schema: {}",
                replacements.schema
            ));
        }
        if replacements.schema != "nrl.replacements/0"
            || replacements.objects.is_empty()
            || replacements.objects.len() > MAX_OBJECTS
            || replacements.reason.trim().is_empty()
            || replacements.reason.len() > 4096
        {
            return Err("invalid replacement set".into());
        }
        validate_sources(std::slice::from_ref(&replacements.source))?;
        let mut objects = self.objects.clone();
        let mut entries = BTreeMap::new();
        let mut changes = Vec::new();
        for (index, (key, data)) in replacements.objects.into_iter().enumerate() {
            let before = self
                .objects
                .get(&key)
                .ok_or("replacement references an unknown object")?;
            let after = Arc::new(Object::new(data)?);
            if before.summary.kind != after.summary.kind
                || before.summary.input_width != after.summary.input_width
                || before.summary.output_width != after.summary.output_width
            {
                return Err("replacement changes graph boundary semantics".into());
            }
            let entry = record(index, &after);
            let before_sha256 = storage::hash(&before.bytes);
            if entry.sha256 == before_sha256 {
                return Err("replacement is unchanged".into());
            }
            changes.push(Change {
                object: key.clone(),
                operation: "manual-parameter-replacement/0".into(),
                base_model_id: self.model_id.clone(),
                before_sha256,
                after_sha256: entry.sha256.clone(),
                parameters: entry.parameters.clone(),
                reason: replacements.reason.clone(),
                source_identifier: replacements.source.identifier.clone(),
            });
            entries.insert(key.clone(), entry);
            objects.insert(key, after);
        }
        let mut sources = self.manifest.sources.clone();
        if !sources.contains(&replacements.source) {
            sources.push(replacements.source);
        }
        let manifest = Manifest {
            schema: "nrl.model/0".into(),
            kind: "overlay".into(),
            descriptor: self.manifest.descriptor.clone(),
            execution: self.manifest.execution.clone(),
            base: Some(Dependency {
                model_id: self.model_id.clone(),
            }),
            objects: entries,
            sources,
            history: self.manifest.history.clone(),
            changes,
        };
        Self::assemble(manifest, objects, self.package_bytes)
    }
    pub fn standalone(&self) -> Result<Self> {
        let mut manifest = self.manifest.clone();
        manifest.kind = "standalone".into();
        manifest.base = None;
        manifest.history.extend(manifest.changes.drain(..));
        manifest.objects = self
            .objects
            .iter()
            .enumerate()
            .map(|(i, (k, v))| (k.clone(), record(i, v)))
            .collect();
        Self::assemble(manifest, self.objects.clone(), 0)
    }
    pub fn save(&self, directory: &Path) -> Result<()> {
        let header = serde_json::to_vec_pretty(&self.manifest).map_err(|e| e.to_string())?;
        fs::create_dir(directory).map_err(|e| e.to_string())?;
        for (key, entry) in &self.manifest.objects {
            storage::write_new(&directory.join(&entry.file), &self.objects[key].bytes)?;
        }
        storage::write_new(&directory.join("manifest.json"), &header)
    }
    pub fn run_to_file(&self, input: &[f64], path: &Path) -> Result<()> {
        let output = self.forward(input)?;
        let graph = &self.manifest.descriptor.configuration;
        let rows = input.len() / graph.input_width;
        let width = output.len() / rows;
        let result = serde_json::json!({"schema":"nrl.model-output/0","model_id":self.model_id,"rows":rows,"output_width":width,"output":output});
        let bytes = serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?;
        if bytes.len() > 32 << 20 {
            return Err("model output byte limit".into());
        }
        storage::write_new(path, &bytes)
    }
    pub fn info(&self) -> Result<serde_json::Value> {
        let c = check_graph(
            &self.manifest.descriptor,
            &self.manifest.execution,
            &self.objects,
        )?;
        Ok(
            serde_json::json!({"schema":"nrl.model-info/0","model_id":self.model_id,"kind":self.manifest.kind,"family":self.manifest.descriptor.family,"nodes":self.manifest.descriptor.configuration.nodes.len(),"objects":self.objects.len(),"local_objects":self.manifest.objects.len(),"numeric_values":c.numeric_values,"trainable_values":c.trainable_values,"numeric_bytes":c.numeric_values*8,"package_bytes":self.package_bytes,"dependency_bytes":self.dependency_bytes,"total_required_package_bytes":self.package_bytes+self.dependency_bytes,"activation_values_per_row":c.values_per_row,"input_width":self.manifest.descriptor.configuration.input_width,"output_width":c.widths[&self.manifest.descriptor.configuration.output],"tied_objects":self.manifest.descriptor.tied_objects,"native_execution":false}),
        )
    }
}

pub fn load(directory: &Path, base: Option<&Model>) -> Result<Model> {
    let bytes = storage::read_regular(&directory.join("manifest.json"), MAX_MANIFEST_BYTES)?;
    let value = import_json::parse(&bytes, MAX_MANIFEST_BYTES)?;
    require_schema(&value, "nrl.model/0")?;
    let manifest: Manifest = serde_json::from_value(value).map_err(|e| e.to_string())?;
    if manifest.objects.is_empty() || manifest.objects.len() > MAX_OBJECTS {
        return Err("model object count limit".into());
    }
    let (mut objects, dependency_bytes) = match manifest.kind.as_str() {
        "standalone" => {
            if manifest.base.is_some() || !manifest.changes.is_empty() || base.is_some() {
                return Err("standalone model has an unexpected base or changes".into());
            }
            (BTreeMap::new(), 0)
        }
        "overlay" => {
            let base = base.ok_or("overlay requires its explicitly supplied base model")?;
            if base.manifest.kind != "standalone"
                || manifest
                    .base
                    .as_ref()
                    .is_none_or(|d| d.model_id != base.model_id)
            {
                return Err("overlay base identity mismatch or unsupported chain".into());
            }
            if manifest.descriptor != base.manifest.descriptor
                || manifest.execution != base.manifest.execution
                || manifest.history != base.manifest.history
                || !manifest.sources.starts_with(&base.manifest.sources)
            {
                return Err("overlay changed graph/profile or discarded base provenance".into());
            }
            (base.objects.clone(), base.package_bytes)
        }
        other => return Err(format!("unsupported model package kind: {other}")),
    };
    let mut total = bytes.len();
    for (index, (key, entry)) in manifest.objects.iter().enumerate() {
        if !id(key)
            || entry.file != format!("object-{index:04}.json")
            || !storage::valid_hash(&entry.sha256)
            || entry.bytes > MAX_ARTIFACT_BYTES
        {
            return Err("invalid model object path or record".into());
        }
        total = count_add(total, entry.bytes, MAX_MODEL_BYTES)?;
        let raw = storage::read_regular(&directory.join(&entry.file), MAX_ARTIFACT_BYTES)?;
        if raw.len() != entry.bytes || storage::hash(&raw) != entry.sha256 {
            return Err("model object checksum or length mismatch".into());
        }
        let data = decode_object(import_json::parse(&raw, MAX_ARTIFACT_BYTES)?)?;
        let summary = data.summary()?;
        if summary != entry.parameters {
            return Err("model object parameter record mismatch".into());
        }
        if manifest.kind == "overlay" {
            let before = objects
                .get(key)
                .ok_or("overlay references unknown base object")?;
            if before.summary.kind != summary.kind
                || before.summary.input_width != summary.input_width
                || before.summary.output_width != summary.output_width
            {
                return Err("overlay changes graph boundary semantics".into());
            }
            let change = manifest
                .changes
                .get(index)
                .ok_or("missing overlay change record")?;
            if change.object != *key
                || change.before_sha256 != storage::hash(&before.bytes)
                || change.after_sha256 != entry.sha256
                || change.parameters != summary
                || change.base_model_id != base.unwrap().model_id
                || !manifest
                    .sources
                    .iter()
                    .any(|s| s.identifier == change.source_identifier)
                || change.before_sha256 == change.after_sha256
            {
                return Err("overlay change record mismatch".into());
            }
        }
        objects.insert(
            key.clone(),
            Arc::new(Object {
                data,
                bytes: raw,
                summary,
            }),
        );
    }
    if manifest.kind == "overlay" && manifest.changes.len() != manifest.objects.len() {
        return Err("overlay change count mismatch".into());
    }
    let mut model = Model::assemble(manifest, objects, dependency_bytes)?;
    model.package_bytes = total;
    Ok(model)
}
