use crate::{
    model::{self, Definition as GraphDefinition, Model, Source},
    research, storage, Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};
const MAX_FILE: usize = 1 << 20;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub schema: String,
    pub tokens: Vec<String>,
    pub dimension: usize,
    pub values: Vec<f64>,
}
impl Table {
    pub fn validate(&self) -> Result<()> {
        if self.schema != "nrl.embedding-table/0"
            || self.tokens.is_empty()
            || self.tokens.len() > 1024
            || self.dimension == 0
            || self.dimension > 128
            || self.tokens.len() * self.dimension > 16384
            || self.values.len() != self.tokens.len() * self.dimension
            || self
                .tokens
                .iter()
                .any(|t| t.is_empty() || t.len() > 64 || !t.bytes().all(|b| b.is_ascii_graphic()))
            || self.tokens.iter().collect::<BTreeSet<_>>().len() != self.tokens.len()
        {
            return Err("unsupported embedding vocabulary/table profile".into());
        }
        research::finite(&self.values)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub schema: String,
    pub table: Table,
    pub projection: GraphDefinition,
    pub max_input_tokens: usize,
    pub sources: Vec<Source>,
}
impl Definition {
    pub fn load(path: &Path) -> Result<Self> {
        research::parse(&research::read(path)?, "nrl.embedding-definition/0")
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    bytes: usize,
    sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    family: String,
    adapter_version: usize,
    preprocessor: String,
    pooling: String,
    normalization: String,
    state: String,
    max_input_tokens: usize,
    table: FileRecord,
    projection_model_id: String,
    sources: Vec<Source>,
    previous_projection_ids: Vec<String>,
}
pub struct Encoder {
    table: Table,
    projection: Model,
    manifest: Manifest,
    id: String,
}
fn table_bytes(table: &Table) -> Result<Vec<u8>> {
    table.validate()?;
    let b = research::bytes(table)?;
    if b.len() > MAX_FILE {
        return Err("embedding table file limit".into());
    }
    Ok(b)
}
fn append(sources: &mut Vec<Source>, extra: &[Source]) -> Result<()> {
    for s in extra {
        if !sources
            .iter()
            .any(|v| serde_json::to_value(v).unwrap() == serde_json::to_value(s).unwrap())
        {
            sources.push(s.clone());
        }
    }
    model::validate_sources(sources)
}
impl Encoder {
    pub fn from_definition(d: Definition) -> Result<Self> {
        if d.schema != "nrl.embedding-definition/0" {
            return Err("unsupported embedding definition version".into());
        }
        let projection = Model::from_definition(d.projection)?;
        let bytes = table_bytes(&d.table)?;
        let mut sources = d.sources;
        model::validate_sources(&sources)?;
        append(&mut sources, &projection.manifest().sources)?;
        let manifest = Manifest {
            schema: "nrl.embedding/0".into(),
            family: "nrl.mean-embedding".into(),
            adapter_version: 0,
            preprocessor: "exact-ascii-whitespace/0".into(),
            pooling: "arithmetic-mean/0".into(),
            normalization: "l2-reject-norm-at-most-1e-12/0".into(),
            state: "stateless".into(),
            max_input_tokens: d.max_input_tokens,
            table: FileRecord {
                bytes: bytes.len(),
                sha256: storage::hash(&bytes),
            },
            projection_model_id: projection.model_id().into(),
            sources,
            previous_projection_ids: Vec::new(),
        };
        Self::checked(d.table, projection, manifest)
    }
    fn checked(table: Table, projection: Model, manifest: Manifest) -> Result<Self> {
        let bytes = table_bytes(&table)?;
        model::validate_sources(&manifest.sources)?;
        let info = projection.info()?;
        if manifest.schema != "nrl.embedding/0"
            || manifest.family != "nrl.mean-embedding"
            || manifest.adapter_version != 0
            || manifest.preprocessor != "exact-ascii-whitespace/0"
            || manifest.pooling != "arithmetic-mean/0"
            || manifest.normalization != "l2-reject-norm-at-most-1e-12/0"
            || manifest.state != "stateless"
            || !(1..=128).contains(&manifest.max_input_tokens)
            || manifest.projection_model_id != projection.model_id()
            || projection.manifest().kind != "standalone"
            || manifest.table.bytes != bytes.len()
            || manifest.table.sha256 != storage::hash(&bytes)
            || info["input_width"].as_u64() != Some(table.dimension as u64)
            || info["output_width"]
                .as_u64()
                .is_none_or(|n| n == 0 || n > 128)
            || manifest.previous_projection_ids.len() > 32
            || manifest
                .previous_projection_ids
                .iter()
                .any(|s| !storage::valid_hash(s))
        {
            return Err("embedding manifest/table/projection contract mismatch".into());
        }
        let id = research::id(&manifest)?;
        let encoder = Self {
            table,
            projection,
            manifest,
            id,
        };
        encoder.info()?;
        Ok(encoder)
    }
    pub fn model_id(&self) -> &str {
        &self.id
    }
    pub fn projection(&self) -> &Model {
        &self.projection
    }
    pub fn max_input_tokens(&self) -> usize {
        self.manifest.max_input_tokens
    }
    pub fn output_width(&self) -> Result<usize> {
        self.projection.info()?["output_width"]
            .as_u64()
            .map(|v| v as usize)
            .ok_or_else(|| "missing embedding output width".into())
    }
    pub fn input_content_id(&self) -> Result<String> {
        research::id(
            &serde_json::json!({"table":self.manifest.table,"max_input_tokens":self.manifest.max_input_tokens,"preprocessor":self.manifest.preprocessor,"pooling":self.manifest.pooling,"normalization":self.manifest.normalization,"input_width":self.table.dimension,"output_width":self.output_width()?}),
        )
    }
    pub fn input_contract_id(&self) -> Result<String> {
        research::id(
            &serde_json::json!({"tokens":self.table.tokens,"lookup_width":self.table.dimension,"max_input_tokens":self.max_input_tokens(),"preprocessor":self.manifest.preprocessor,"pooling":self.manifest.pooling,"normalization":self.manifest.normalization,"output_width":self.output_width()?}),
        )
    }
    pub fn definition(&self) -> Definition {
        let m = self.projection.manifest();
        Definition {
            schema: "nrl.embedding-definition/0".into(),
            table: self.table.clone(),
            projection: GraphDefinition {
                schema: "nrl.model-definition/0".into(),
                descriptor: m.descriptor.clone(),
                execution: m.execution.clone(),
                sources: m.sources.clone(),
                objects: m
                    .objects
                    .keys()
                    .map(|k| (k.clone(), self.projection.object(k).unwrap().clone()))
                    .collect(),
            },
            max_input_tokens: self.max_input_tokens(),
            sources: self.manifest.sources.clone(),
        }
    }
    pub fn parameters(&self) -> Result<crate::optimizer::Tensors> {
        let mut p = crate::optimizer::model_parameters(&self.projection)?;
        p.insert("table".into(), self.table.values.clone());
        Ok(p)
    }
    pub fn with_training_parameters(
        &self,
        p: &crate::optimizer::Tensors,
        sources: &[Source],
    ) -> Result<Self> {
        if p.keys().ne(self.parameters()?.keys()) {
            return Err("embedding parameter inventory mismatch".into());
        }
        let mut table = self.table.clone();
        table.values = p.get("table").ok_or("missing lookup table")?.clone();
        let projection = crate::optimizer::replace_model(&self.projection, p)?;
        let raw = table_bytes(&table)?;
        let mut manifest = self.manifest.clone();
        manifest.table = FileRecord {
            bytes: raw.len(),
            sha256: storage::hash(&raw),
        };
        manifest.projection_model_id = projection.model_id().into();
        append(&mut manifest.sources, sources)?;
        Self::checked(table, projection, manifest)
    }
    pub fn backward_tokens(
        &self,
        tokens: &[usize],
        dy: &[f64],
        max_training_values: usize,
    ) -> Result<Backward> {
        let output = self.encode_tokens(tokens)?;
        if dy.len() != output.len() {
            return Err("embedding output adjoint shape".into());
        }
        research::finite(dy)?;
        let mut mean = vec![0.0; self.table.dimension];
        for t in tokens {
            for (j, x) in mean.iter_mut().enumerate() {
                *x += self.table.values[t * self.table.dimension + j];
            }
        }
        for x in &mut mean {
            *x /= tokens.len() as f64;
        }
        let raw = self.projection.forward(&mean)?;
        let norm = raw.iter().map(|x| x * x).sum::<f64>().sqrt();
        let dot = dy.iter().zip(&output).map(|(a, b)| a * b).sum::<f64>();
        let gradient: Vec<_> = dy
            .iter()
            .zip(&output)
            .map(|(d, y)| (d - y * dot) / norm)
            .collect();
        let backward = self
            .projection
            .backward(&mean, &gradient, max_training_values)?;
        let mut table = vec![0.0; self.table.values.len()];
        for t in tokens {
            for (j, g) in backward.input_gradient.iter().enumerate() {
                table[t * self.table.dimension + j] += g / tokens.len() as f64;
            }
        }
        research::finite(&table)?;
        let mut gradients = crate::optimizer::model_gradients(&backward.gradients);
        gradients.insert("table".into(), table);
        Ok(Backward { output, gradients })
    }
    pub fn tokenize(&self, text: &str) -> Result<Vec<usize>> {
        if text.is_empty() || text.len() > 4096 || !text.is_ascii() {
            return Err("embedding text must be bounded ASCII".into());
        }
        let mut result = Vec::new();
        for token in text.split_ascii_whitespace() {
            if result.len() >= self.manifest.max_input_tokens {
                return Err("embedding token budget exceeded; no truncation".into());
            }
            result.push(
                self.table
                    .tokens
                    .iter()
                    .position(|s| s == token)
                    .ok_or("unknown exact embedding vocabulary token")?,
            );
        }
        if result.is_empty() {
            return Err("empty embedding token sequence".into());
        }
        Ok(result)
    }
    pub fn encode_tokens(&self, tokens: &[usize]) -> Result<Vec<f64>> {
        if tokens.is_empty()
            || tokens.len() > self.manifest.max_input_tokens
            || tokens.iter().any(|t| *t >= self.table.tokens.len())
        {
            return Err("embedding token ID/length bound".into());
        }
        let mut mean = vec![0.0; self.table.dimension];
        for t in tokens {
            for (x, w) in mean
                .iter_mut()
                .zip(&self.table.values[t * self.table.dimension..(t + 1) * self.table.dimension])
            {
                *x += w;
            }
        }
        for x in &mut mean {
            *x /= tokens.len() as f64;
        }
        research::finite(&mean)?;
        let mut vector = self.projection.forward(&mean)?;
        let norm = vector.iter().map(|x| x * x).sum::<f64>().sqrt();
        if !norm.is_finite() || norm <= 1e-12 {
            return Err("embedding projection has zero/non-finite norm".into());
        }
        for x in &mut vector {
            *x /= norm;
        }
        research::finite(&vector)?;
        Ok(vector)
    }
    pub fn encode(&self, text: &str) -> Result<Vec<f64>> {
        self.encode_tokens(&self.tokenize(text)?)
    }
    pub fn encode_to_file(&self, text: &str, path: &Path) -> Result<serde_json::Value> {
        let tokens = self.tokenize(text)?;
        let vector = self.encode_tokens(&tokens)?;
        let r = serde_json::json!({"schema":"nrl.embedding-vector/0","model_id":self.model_id(),"tokens":tokens,"vector":vector,"native_execution":false});
        research::save(path, &r)?;
        Ok(r)
    }
    pub fn prune_projection(&self, object: &str, width: usize, source: Source) -> Result<Self> {
        let original = match self
            .projection
            .object(object)
            .ok_or("unknown embedding projection object")?
        {
            model::ObjectData::Gated(a) => a,
            _ => {
                return Err("embedding pruning requires a supported gated projection object".into())
            }
        };
        let block = crate::prune(original, width, None)?;
        let replacements=model::Replacements{schema:"nrl.replacements/0".into(),source:source.clone(),reason:"Explicit weight-energy pruned embedding projection control; no quality or speed gain inferred".into(),objects:std::collections::BTreeMap::from([(object.to_owned(),model::ObjectData::Gated(block))])};
        self.replace_projection(self.projection.overlay(replacements)?.standalone()?, source)
    }
    pub fn replace_projection(&self, projection: Model, source: Source) -> Result<Self> {
        if projection.info()?["output_width"].as_u64() != Some(self.output_width()? as u64) {
            return Err("embedding projection external boundary changed".into());
        }
        let mut manifest = self.manifest.clone();
        manifest
            .previous_projection_ids
            .push(self.projection.model_id().into());
        manifest.projection_model_id = projection.model_id().into();
        append(&mut manifest.sources, &projection.manifest().sources)?;
        append(&mut manifest.sources, &[source])?;
        Self::checked(self.table.clone(), projection, manifest)
    }
    pub fn info(&self) -> Result<serde_json::Value> {
        let p = self.projection.info()?;
        let manifest = research::bytes(&self.manifest)?;
        if manifest.len() > MAX_FILE {
            return Err("embedding manifest byte limit".into());
        }
        let projection_bytes = research::bytes(self.projection.manifest())?.len()
            + self
                .projection
                .manifest()
                .objects
                .values()
                .map(|v| v.bytes)
                .sum::<usize>();
        let package_bytes = manifest.len() + self.manifest.table.bytes + projection_bytes;
        Ok(
            serde_json::json!({"schema":"nrl.embedding-info/0","model_id":self.id,"family":self.manifest.family,"adapter_version":0,"vocabulary_size":self.table.tokens.len(),"lookup_width":self.table.dimension,"output_width":self.output_width()?,"max_input_tokens":self.max_input_tokens(),"preprocessor":self.manifest.preprocessor,"pooling":self.manifest.pooling,"normalization":self.manifest.normalization,"state":"stateless","supported_tasks":["embedding-retrieval/0"],"projection":p,"numeric_values":self.table.values.len()+p["numeric_values"].as_u64().unwrap_or(0) as usize,"package_bytes":package_bytes,"transform_scope":"existing checked projection objects only; lookup/pooling/normalization remain unchanged","training_scope":"analytic lookup/mean/projection/L2 backward; bounded full-batch contrastive AdamW; query-split validation selection","native_execution":false}),
        )
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.info()?;
        fs::create_dir(path).map_err(|e| e.to_string())?;
        self.projection.save(&path.join("projection"))?;
        storage::write_new(&path.join("table.json"), &table_bytes(&self.table)?)?;
        research::save(&path.join("manifest.json"), &self.manifest)
    }
}
pub struct Backward {
    pub output: Vec<f64>,
    pub gradients: crate::optimizer::Tensors,
}
pub fn load(path: &Path) -> Result<Encoder> {
    let raw = storage::read_regular(&path.join("manifest.json"), MAX_FILE)?;
    let manifest: Manifest = research::parse(&raw, "nrl.embedding/0")?;
    if raw != research::bytes(&manifest)? {
        return Err("embedding manifest must have canonical package encoding".into());
    }
    if manifest.table.bytes > MAX_FILE || !storage::valid_hash(&manifest.table.sha256) {
        return Err("embedding table record bounds/hash".into());
    }
    let bytes = storage::read_regular(&path.join("table.json"), MAX_FILE)?;
    if bytes.len() != manifest.table.bytes || storage::hash(&bytes) != manifest.table.sha256 {
        return Err("embedding table checksum mismatch".into());
    }
    let table: Table = research::parse(&bytes, "nrl.embedding-table/0")?;
    if bytes != table_bytes(&table)? {
        return Err("embedding table must have canonical package encoding".into());
    }
    let projection = model::load(&path.join("projection"), None)?;
    if storage::read_regular(&path.join("projection/manifest.json"), MAX_FILE)?
        != research::bytes(projection.manifest())?
    {
        return Err("embedding projection manifest must have canonical package encoding".into());
    }
    Encoder::checked(table, projection, manifest)
}
