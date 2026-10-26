use crate::{descriptor::BlockDescriptor, import_json, model::Source, storage, Artifact, Result};
use nrl_repr::{Activation, Block};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

mod tokenizer;
pub use tokenizer::Tokenizer;
pub const MAX_BYTES: usize = 16 << 20;
pub const MAX_VALUES: usize = 1 << 20;
pub const MAX_TRACE_VALUES: usize = 1 << 20;
const POLICY: &str = "cpu-scalar-f64-source-f32-stored-rope/0";
fn finite(values: &[f64]) -> Result<()> {
    nrl_core::finite(values).map_err(|_| "nonfinite decoder numerical value".into())
}
fn numerical<T>(v: nrl_core::Result<T>) -> Result<T> {
    v.map_err(|e| format!("decoder numerical contract: {e:?}"))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub dim: usize,
    pub hidden: usize,
    pub layers: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub vocab: usize,
    pub context: usize,
    pub tied_head: bool,
}
impl Config {
    fn validate(&self) -> Result<()> {
        if self.dim == 0
            || self.dim > 128
            || self.hidden == 0
            || self.hidden > 512
            || self.layers == 0
            || self.layers > 12
            || self.heads == 0
            || self.heads > 16
            || self.kv_heads == 0
            || self.heads % self.kv_heads != 0
            || self.dim % self.heads != 0
            || (self.dim / self.heads) % 2 != 0
            || !(259..=1024).contains(&self.vocab)
            || self.context == 0
            || self.context > 512
            || self.context * self.kv_width() * self.layers * 2 > MAX_VALUES
        {
            return Err("unsupported decoder configuration/shape".into());
        }
        Ok(())
    }
    fn kv_width(&self) -> usize {
        self.dim / self.heads * self.kv_heads
    }
}
#[derive(Clone)]
pub struct DenseMlp {
    pub dim: usize,
    pub hidden: usize,
    pub gate: Arc<Vec<f64>>,
    pub up: Arc<Vec<f64>>,
    pub down: Arc<Vec<f64>>,
}
impl DenseMlp {
    pub fn block(&self) -> Result<Block<'_>> {
        numerical(Block::new(
            numerical(nrl_core::Shape::new(self.dim, self.hidden, self.dim))?,
            &self.gate,
            &self.up,
            &self.down,
            Activation::Silu,
            nrl_repr::bias::Biases::default(),
        ))
    }
    pub fn forward(&self, input: &[f64]) -> Result<Vec<f64>> {
        if input.is_empty() || input.len() % self.dim != 0 || input.len() > MAX_VALUES {
            return Err("dense MLP input shape/limit".into());
        }
        let block = self.block()?;
        let mut scratch = vec![0.0; block.scratch_len()];
        let mut out = vec![0.0; input.len()];
        for (x, y) in input
            .chunks_exact(self.dim)
            .zip(out.chunks_exact_mut(self.dim))
        {
            numerical(block.forward(x, y, &mut scratch))?;
        }
        Ok(out)
    }
    pub fn descriptor(&self) -> BlockDescriptor {
        BlockDescriptor {
            kind: "gated-feed-forward/0".into(),
            source_activation: "silu".into(),
            dtype: "f64".into(),
            layout: "row-major".into(),
            state: "stateless".into(),
            parameter_sharing: "independent".into(),
        }
    }
}
#[derive(Clone)]
struct Layer {
    norm_att: Arc<Vec<f64>>,
    q: Arc<Vec<f64>>,
    k: Arc<Vec<f64>>,
    v: Arc<Vec<f64>>,
    o: Arc<Vec<f64>>,
    norm_ff: Arc<Vec<f64>>,
    mlp: DenseMlp,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    file: String,
    sha256: String,
    bytes: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    adapter: String,
    numerical_policy: String,
    config: Config,
    weights: FileRecord,
    tokenizer: FileRecord,
    patches: BTreeMap<usize, FileRecord>,
    sources: Vec<Source>,
}
#[derive(Clone)]
pub struct Decoder {
    config: Config,
    layers: Arc<Vec<Layer>>,
    embedding: Arc<Vec<f64>>,
    head: Arc<Vec<f64>>,
    norm: Arc<Vec<f64>>,
    cos: Arc<Vec<f64>>,
    sin: Arc<Vec<f64>>,
    raw: Arc<Vec<u8>>,
    token_bytes: Arc<Vec<u8>>,
    pub tokenizer: Tokenizer,
    patches: BTreeMap<usize, Arc<Artifact>>,
    manifest: Manifest,
    id: String,
    base_values: usize,
}
#[derive(Clone)]
struct LayerState {
    keys: Vec<f64>,
    values: Vec<f64>,
}
#[derive(Clone)]
pub struct State {
    model_id: String,
    position: usize,
    layers: Vec<LayerState>,
}
impl State {
    pub fn position(&self) -> usize {
        self.position
    }
}
pub struct LayerTrace {
    pub input: Vec<f64>,
    pub output: Vec<f64>,
}
pub struct Trace {
    pub logits: Vec<f64>,
    pub layers: Vec<LayerTrace>,
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn values(&mut self, count: usize) -> Result<Arc<Vec<f64>>> {
        let end = self
            .offset
            .checked_add(count.checked_mul(4).ok_or("weight count overflow")?)
            .filter(|end| *end <= self.bytes.len())
            .ok_or("truncated decoder weights")?;
        let v: Vec<_> = self.bytes[self.offset..end]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()) as f64)
            .collect();
        finite(&v)?;
        self.offset = end;
        Ok(Arc::new(v))
    }
}
fn record(file: &str, bytes: &[u8]) -> FileRecord {
    FileRecord {
        file: file.into(),
        sha256: storage::hash(bytes),
        bytes: bytes.len(),
    }
}
fn manifest_id(m: &Manifest) -> Result<String> {
    Ok(storage::hash(
        import_json::canonical_compact(&serde_json::to_value(m).map_err(|e| e.to_string())?)
            .as_bytes(),
    ))
}
fn matrix(weights: &[f64], input: &[f64], rows: usize) -> Result<Vec<f64>> {
    if weights.len() != rows * input.len() {
        return Err("decoder matrix shape".into());
    }
    let out: Vec<_> = weights
        .chunks_exact(input.len())
        .map(|row| row.iter().zip(input).map(|(a, b)| a * b).sum())
        .collect();
    finite(&out)?;
    Ok(out)
}
fn norm(x: &[f64], weights: &[f64]) -> Result<Vec<f64>> {
    let energy = x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64;
    finite(&[energy])?;
    let scale = 1.0 / (energy + 1e-5).sqrt();
    let out: Vec<_> = x.iter().zip(weights).map(|(x, w)| x * scale * w).collect();
    finite(&out)?;
    Ok(out)
}
impl Decoder {
    pub fn from_bytes(raw: Vec<u8>, token_bytes: Vec<u8>, sources: Vec<Source>) -> Result<Self> {
        crate::model::validate_sources(&sources)?;
        if raw.len() < 28
            || raw.len() > MAX_BYTES
            || (raw.len() - 28) % 4 != 0
            || (raw.len() - 28) / 4 > MAX_VALUES
        {
            return Err("decoder weight byte/value limit".into());
        }
        let h: Vec<_> = raw[..28]
            .chunks_exact(4)
            .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        if h.iter().enumerate().any(|(i, v)| *v <= 0 && i != 5) || h[5] == 0 || h[5] == i32::MIN {
            return Err("invalid decoder header".into());
        }
        let config = Config {
            dim: h[0] as usize,
            hidden: h[1] as usize,
            layers: h[2] as usize,
            heads: h[3] as usize,
            kv_heads: h[4] as usize,
            vocab: h[5].unsigned_abs() as usize,
            context: h[6] as usize,
            tied_head: h[5] > 0,
        };
        config.validate()?;
        let mut r = Reader {
            bytes: &raw,
            offset: 28,
        };
        let c = &config;
        let embedding = r.values(c.vocab * c.dim)?;
        let mut read_layers = |size| {
            (0..c.layers)
                .map(|_| r.values(size))
                .collect::<Result<Vec<_>>>()
        };
        let na = read_layers(c.dim)?;
        let q = read_layers(c.dim * c.dim)?;
        let k = read_layers(c.kv_width() * c.dim)?;
        let v = read_layers(c.kv_width() * c.dim)?;
        let o = read_layers(c.dim * c.dim)?;
        let nf = read_layers(c.dim)?;
        let gate = read_layers(c.hidden * c.dim)?;
        let down = read_layers(c.dim * c.hidden)?;
        let up = read_layers(c.hidden * c.dim)?;
        let final_norm = r.values(c.dim)?;
        let frequencies = c.context * (c.dim / c.heads) / 2;
        let cos = r.values(frequencies)?;
        let sin = r.values(frequencies)?;
        let head = if c.tied_head {
            embedding.clone()
        } else {
            r.values(c.vocab * c.dim)?
        };
        if r.offset != raw.len() {
            return Err("trailing or inconsistent decoder weights".into());
        }
        let base_values = (raw.len() - 28) / 4 - 2 * frequencies;
        let layers = (0..c.layers)
            .map(|i| Layer {
                norm_att: na[i].clone(),
                q: q[i].clone(),
                k: k[i].clone(),
                v: v[i].clone(),
                o: o[i].clone(),
                norm_ff: nf[i].clone(),
                mlp: DenseMlp {
                    dim: c.dim,
                    hidden: c.hidden,
                    gate: gate[i].clone(),
                    up: up[i].clone(),
                    down: down[i].clone(),
                },
            })
            .collect();
        let tokenizer = Tokenizer::parse(&token_bytes, c.vocab)?;
        let manifest = Manifest {
            schema: "nrl.decoder/0".into(),
            adapter: "llama2c-f32-v0/0".into(),
            numerical_policy: POLICY.into(),
            config: config.clone(),
            weights: record("weights.bin", &raw),
            tokenizer: record("tokenizer.bin", &token_bytes),
            patches: BTreeMap::new(),
            sources,
        };
        let id = manifest_id(&manifest)?;
        let model = Self {
            config,
            layers: Arc::new(layers),
            embedding,
            head,
            norm: final_norm,
            cos,
            sin,
            raw: Arc::new(raw),
            token_bytes: Arc::new(token_bytes),
            tokenizer,
            patches: BTreeMap::new(),
            manifest,
            id,
            base_values,
        };
        if model.package_bytes()? > MAX_BYTES {
            return Err("decoder package byte budget".into());
        }
        Ok(model)
    }

    pub fn model_id(&self) -> &str {
        &self.id
    }
    pub fn config(&self) -> &Config {
        &self.config
    }
    pub fn sources(&self) -> &[Source] {
        &self.manifest.sources
    }
    pub fn dense(&self, layer: usize) -> Result<&DenseMlp> {
        self.layers
            .get(layer)
            .map(|l| &l.mlp)
            .ok_or("decoder layer outside range".into())
    }
    pub fn patch(&self, layer: usize) -> Option<&Artifact> {
        self.patches.get(&layer).map(AsRef::as_ref)
    }
    pub fn patches(&self) -> BTreeMap<usize, Artifact> {
        self.patches
            .iter()
            .map(|(i, a)| (*i, (**a).clone()))
            .collect()
    }
    pub fn with_patches(
        &self,
        patches: BTreeMap<usize, Artifact>,
        sources: &[Source],
    ) -> Result<Self> {
        let mut next = self.clone();
        next.patches.clear();
        next.manifest.patches.clear();
        for (layer, a) in patches {
            a.validate()?;
            if layer >= self.config.layers
                || a.shape[0] != self.config.dim
                || a.shape[2] != self.config.dim
            {
                return Err("decoder patch layer/boundary mismatch".into());
            }
            let bytes = serde_json::to_vec_pretty(&a).map_err(|e| e.to_string())?;
            next.manifest
                .patches
                .insert(layer, record(&format!("block-{layer:04}.json"), &bytes));
            next.patches.insert(layer, Arc::new(a));
        }
        for source in sources {
            if !next.manifest.sources.contains(source) {
                next.manifest.sources.push(source.clone());
            }
        }
        crate::model::validate_sources(&next.manifest.sources)?;
        if next.package_bytes()? > MAX_BYTES {
            return Err("decoder package byte budget".into());
        }
        next.id = manifest_id(&next.manifest)?;
        Ok(next)
    }
    pub fn state(&self) -> State {
        State {
            model_id: self.id.clone(),
            position: 0,
            layers: (0..self.config.layers)
                .map(|_| LayerState {
                    keys: Vec::new(),
                    values: Vec::new(),
                })
                .collect(),
        }
    }
    pub fn step(&self, state: &mut State, token: usize) -> Result<Vec<f64>> {
        let mut next = state.clone();
        let (logits, _) = self.step_inner(&mut next, token, false)?;
        *state = next;
        Ok(logits)
    }
    fn step_inner(
        &self,
        state: &mut State,
        token: usize,
        trace: bool,
    ) -> Result<(Vec<f64>, Vec<LayerTrace>)> {
        let c = &self.config;
        if state.model_id != self.id || state.position >= c.context || token >= c.vocab {
            return Err("decoder state binding, context or token limit".into());
        }
        let position = state.position;
        let head_width = c.dim / c.heads;
        let kv = c.kv_width();
        let mut x = self.embedding[token * c.dim..(token + 1) * c.dim].to_vec();
        let mut traces = Vec::new();
        for (index, layer) in self.layers.iter().enumerate() {
            let attention_input = norm(&x, &layer.norm_att)?;
            let mut q = matrix(&layer.q, &attention_input, c.dim)?;
            let mut k = matrix(&layer.k, &attention_input, kv)?;
            let v = matrix(&layer.v, &attention_input, kv)?;
            for values in [&mut q, &mut k] {
                for head in values.chunks_exact_mut(head_width) {
                    for (pair, coords) in head.chunks_exact_mut(2).enumerate() {
                        let cos = self.cos[position * head_width / 2 + pair];
                        let sin = self.sin[position * head_width / 2 + pair];
                        let (a, b) = (coords[0], coords[1]);
                        coords[0] = a * cos - b * sin;
                        coords[1] = a * sin + b * cos;
                    }
                }
                finite(values)?;
            }
            let cache = &mut state.layers[index];
            cache.keys.extend_from_slice(&k);
            cache.values.extend_from_slice(&v);
            let mut attended = vec![0.0; c.dim];
            for h in 0..c.heads {
                let kh = h / (c.heads / c.kv_heads);
                let query = &q[h * head_width..(h + 1) * head_width];
                let mut scores: Vec<_> = (0..=position)
                    .map(|p| {
                        query
                            .iter()
                            .zip(
                                &cache.keys
                                    [p * kv + kh * head_width..p * kv + (kh + 1) * head_width],
                            )
                            .map(|(a, b)| a * b)
                            .sum::<f64>()
                            / (head_width as f64).sqrt()
                    })
                    .collect();
                finite(&scores)?;
                let maximum = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let mut sum = 0.0;
                for s in &mut scores {
                    *s = (*s - maximum).exp();
                    sum += *s;
                }
                for (p, score) in scores.iter().enumerate() {
                    for j in 0..head_width {
                        attended[h * head_width + j] +=
                            score / sum * cache.values[p * kv + kh * head_width + j];
                    }
                }
            }
            let projected = matrix(&layer.o, &attended, c.dim)?;
            for (x, y) in x.iter_mut().zip(projected) {
                *x += y;
            }
            finite(&x)?;
            let input = norm(&x, &layer.norm_ff)?;
            let output = if let Some(a) = self.patches.get(&index) {
                a.forward(&input)?
            } else {
                layer.mlp.forward(&input)?
            };
            for (x, y) in x.iter_mut().zip(&output) {
                *x += y;
            }
            finite(&x)?;
            if trace {
                traces.push(LayerTrace { input, output });
            }
        }
        let output = matrix(&self.head, &norm(&x, &self.norm)?, c.vocab)?;
        state.position += 1;
        Ok((output, traces))
    }
    pub fn forward(&self, tokens: &[usize]) -> Result<Vec<f64>> {
        self.run(tokens, false, MAX_TRACE_VALUES).map(|t| t.logits)
    }
    pub fn trace(&self, tokens: &[usize]) -> Result<Trace> {
        self.run(tokens, true, MAX_TRACE_VALUES)
    }
    pub fn trace_with_limit(&self, tokens: &[usize], limit: usize) -> Result<Trace> {
        self.run(tokens, true, limit)
    }
    fn run(&self, tokens: &[usize], capture: bool, limit: usize) -> Result<Trace> {
        let c = &self.config;
        if limit == 0
            || limit > MAX_TRACE_VALUES
            || tokens.is_empty()
            || tokens.len() > c.context
            || tokens.iter().any(|t| *t >= c.vocab)
            || tokens.len() * (c.vocab + if capture { 2 * c.dim * c.layers } else { 0 }) > limit
        {
            return Err("decoder input/context/trace budget".into());
        }
        let mut state = self.state();
        let mut logits = Vec::new();
        let mut layers: Vec<_> = (0..if capture { c.layers } else { 0 })
            .map(|_| LayerTrace {
                input: Vec::new(),
                output: Vec::new(),
            })
            .collect();
        for token in tokens {
            let (out, traces) = self.step_inner(&mut state, *token, capture)?;
            logits.extend(out);
            for (total, row) in layers.iter_mut().zip(traces) {
                total.input.extend(row.input);
                total.output.extend(row.output);
            }
        }
        Ok(Trace { logits, layers })
    }
    fn package_bytes(&self) -> Result<usize> {
        let header = serde_json::to_vec_pretty(&self.manifest)
            .map_err(|e| e.to_string())?
            .len();
        if header > 1 << 20 {
            return Err("decoder manifest byte budget".into());
        }
        Ok(header
            + self.manifest.weights.bytes
            + self.manifest.tokenizer.bytes
            + self
                .manifest
                .patches
                .values()
                .map(|r| r.bytes)
                .sum::<usize>())
    }
    pub fn info(&self) -> Result<serde_json::Value> {
        let summaries = self
            .patches
            .values()
            .map(|a| crate::model::ObjectData::Gated((**a).clone()).summary())
            .collect::<Result<Vec<_>>>()?;
        let effective = self.base_values
            - self.patches.len() * 3 * self.config.dim * self.config.hidden
            + summaries.iter().map(|s| s.trainable_values).sum::<usize>();
        let rope_values = self.cos.len() + self.sin.len();
        let fixed = rope_values
            + summaries
                .iter()
                .map(|s| s.numeric_values - s.trainable_values)
                .sum::<usize>();
        Ok(
            serde_json::json!({"schema":"nrl.decoder-info/0","model_id":self.id,"adapter":self.manifest.adapter,"config":self.config,"numerical_policy":POLICY,"base_parameter_values":self.base_values,"effective_parameter_values":effective,"effective_numeric_values":effective+fixed,"retained_numeric_values":self.base_values+rope_values+summaries.iter().map(|s|s.numeric_values).sum::<usize>(),"patch_layers":self.patches.keys().collect::<Vec<_>>(),"package_bytes":self.package_bytes()?,"retains_original_dense_weights":true,"native_execution":false}),
        )
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        if self.package_bytes()? > MAX_BYTES {
            return Err("decoder package byte budget".into());
        }
        let manifest = serde_json::to_vec_pretty(&self.manifest).map_err(|e| e.to_string())?;
        fs::create_dir(path).map_err(|e| e.to_string())?;
        storage::write_new(&path.join("weights.bin"), &self.raw)?;
        storage::write_new(&path.join("tokenizer.bin"), &self.token_bytes)?;
        for (index, a) in &self.patches {
            storage::write_new(
                &path.join(format!("block-{index:04}.json")),
                &serde_json::to_vec_pretty(a.as_ref()).map_err(|e| e.to_string())?,
            )?;
        }
        storage::write_new(&path.join("manifest.json"), &manifest)
    }
}
pub fn load(path: &Path) -> Result<Decoder> {
    let value = import_json::parse(
        &storage::read_regular(&path.join("manifest.json"), 1 << 20)?,
        1 << 20,
    )?;
    if value["schema"] != "nrl.decoder/0" {
        return Err(format!("unsupported decoder schema: {}", value["schema"]));
    }
    let manifest: Manifest = serde_json::from_value(value).map_err(|e| e.to_string())?;
    if manifest.adapter != "llama2c-f32-v0/0"
        || manifest.numerical_policy != POLICY
        || manifest.patches.len() > 12
    {
        return Err("unsupported decoder adapter/policy or patch count".into());
    }
    let read = |r: &FileRecord, expected: &str, limit| -> Result<Vec<u8>> {
        if r.file != expected || r.bytes > limit {
            return Err("decoder file path/byte limit".into());
        }
        let bytes = storage::read_regular(&path.join(expected), limit)?;
        if bytes.len() != r.bytes || storage::hash(&bytes) != r.sha256 {
            return Err("decoder file length/hash mismatch".into());
        }
        Ok(bytes)
    };
    let mut model = Decoder::from_bytes(
        read(&manifest.weights, "weights.bin", MAX_BYTES)?,
        read(&manifest.tokenizer, "tokenizer.bin", 1 << 20)?,
        manifest.sources.clone(),
    )?;
    if model.config != manifest.config {
        return Err("decoder declared config differs from weights".into());
    }
    let mut patches = BTreeMap::new();
    for (index, r) in &manifest.patches {
        if *index >= model.config.layers {
            return Err("decoder patch layer outside range".into());
        }
        patches.insert(
            *index,
            crate::parse(&read(
                r,
                &format!("block-{index:04}.json"),
                crate::MAX_ARTIFACT_BYTES,
            )?)?,
        );
    }
    model = model.with_patches(patches, &[])?;
    if model.manifest.patches != manifest.patches {
        return Err("decoder patch canonical bytes/identity mismatch".into());
    }
    Ok(model)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDocument {
    schema: String,
    weights_sha256: String,
    tokenizer_sha256: String,
    sources: Vec<Source>,
}
pub fn import(weights: &Path, tokenizer: &Path, source: &Path) -> Result<Decoder> {
    let value = import_json::parse(&storage::read_regular(source, 1 << 20)?, 1 << 20)?;
    if value["schema"] != "nrl.decoder-source/0" {
        return Err(format!(
            "unsupported decoder source schema: {}",
            value["schema"]
        ));
    }
    let source: SourceDocument = serde_json::from_value(value).map_err(|e| e.to_string())?;
    if source.schema != "nrl.decoder-source/0" {
        return Err("unsupported decoder source".into());
    }
    let weights = storage::read_regular(weights, MAX_BYTES)?;
    let tokenizer = storage::read_regular(tokenizer, 1 << 20)?;
    if storage::hash(&weights) != source.weights_sha256
        || storage::hash(&tokenizer) != source.tokenizer_sha256
    {
        return Err("decoder import differs from pinned input hashes".into());
    }
    Decoder::from_bytes(weights, tokenizer, source.sources)
}

impl Decoder {
    pub fn generate(&self, prompt: &[usize], maximum: usize) -> Result<Vec<usize>> {
        if maximum > 64 || prompt.is_empty() || prompt.len() + maximum > self.config.context {
            return Err("generation context/work limit".into());
        }
        let mut state = self.state();
        let mut logits = Vec::new();
        for token in prompt {
            logits = self.step_inner(&mut state, *token, false)?.0;
        }
        let mut generated = Vec::new();
        for _ in 0..maximum {
            let mut best = 0;
            for (index, value) in logits.iter().enumerate().skip(1) {
                if *value > logits[best] {
                    best = index;
                }
            }
            generated.push(best);
            if best == 1 || best == 2 {
                break;
            }
            logits = self.step_inner(&mut state, best, false)?.0;
        }
        Ok(generated)
    }
    pub fn probe_to_file(&self, text: &str, path: &Path) -> Result<serde_json::Value> {
        let tokens = self.tokenizer.encode(text, true, false)?;
        let logits = self.forward(&tokens)?;
        let generated = self.generate(&tokens, 8)?;
        let mut all = tokens.clone();
        all.extend_from_slice(&generated);
        let bytes = self.tokenizer.decode_bytes(&all)?;
        let decoded = String::from_utf8(bytes.clone()).ok();
        let result = serde_json::json!({"schema":"nrl.decoder-output/0","model_id":self.id,"tokens":tokens,"logits":logits,"generated":generated,"decoded_utf8":decoded,"decoded_bytes":bytes,"native_execution":false});
        let output = serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?;
        if output.len() > 32 << 20 {
            return Err("decoder output byte limit".into());
        }
        storage::write_new(path, &output)?;
        Ok(
            serde_json::json!({"schema":"nrl.decoder-probe/0","model_id":self.id,"input_tokens":tokens.len(),"generated_tokens":generated.len(),"decoded_utf8":result["decoded_utf8"],"output_sha256":storage::hash(&output),"native_execution":false}),
        )
    }
}
