use crate::{
    descriptor::BlockDescriptor, import_json, safetensors, Artifact, FactorData, Result,
    SelectionData, SplineData,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub use crate::storage::hash;
use crate::storage::{read_regular, valid_hash, write_new};

pub const MAX_BLOCKS: usize = 32;
pub const MAX_MANIFEST_BYTES: usize = 1 << 20;
pub const MAX_PACKAGE_BYTES: usize = 16 << 20;
pub const QSL_FORMAT: &str = "qwen-spline-lab-overlay-v1";

pub fn artifact_id(fingerprint: &str, blocks: &Value, weights_sha256: &str) -> String {
    let value = json!({"base": fingerprint, "blocks": blocks, "weights": weights_sha256});
    hash(import_json::qsl_canonical(&value).as_bytes())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    representation: String,
    hidden_size: usize,
    original_width: usize,
    width: usize,
    rank: usize,
    knots: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Overlay {
    format: String,
    base: Value,
    blocks: BTreeMap<String, Spec>,
    weights_sha256: String,
    patch_file_bytes: usize,
    patch_storage_dtype: String,
    environment: Value,
    details: Value,
    artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub format: String,
    pub manifest_sha256: String,
    pub weights_sha256: String,
    pub artifact_id: String,
    pub original_patch_bytes: usize,
    pub base: Value,
    pub base_identity_verified: bool,
    pub licence: String,
    pub numerical_policy: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub scope: String,
    pub block_schema: String,
    pub source: Source,
    pub blocks: Vec<Entry>,
    pub expanded_block_bytes: usize,
    pub unrecorded_channel_mappings: Vec<String>,
}

pub struct BlockSet {
    manifest: Manifest,
    blocks: Vec<Artifact>,
    source_manifest: Vec<u8>,
}
impl BlockSet {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn blocks(&self) -> impl Iterator<Item = (&str, &Artifact)> {
        self.manifest
            .blocks
            .iter()
            .zip(&self.blocks)
            .map(|(e, b)| (e.id.as_str(), b))
    }
    pub fn save(&self, directory: &Path) -> Result<()> {
        let mut manifest = self.manifest.clone();
        let serialized: Vec<Vec<u8>> = self
            .blocks
            .iter()
            .map(|b| {
                b.validate()?;
                serde_json::to_vec_pretty(b).map_err(|e| e.to_string())
            })
            .collect::<Result<_>>()?;
        let mut total = self.source_manifest.len();
        manifest.expanded_block_bytes = 0;
        for (entry, bytes) in manifest.blocks.iter_mut().zip(&serialized) {
            total = total
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_PACKAGE_BYTES)
                .ok_or("package byte limit exceeded")?;
            manifest.expanded_block_bytes += bytes.len();
            entry.bytes = bytes.len();
            entry.sha256 = hash(bytes);
        }
        let header = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        if header.len() > MAX_MANIFEST_BYTES
            || total
                .checked_add(header.len())
                .is_none_or(|n| n > MAX_PACKAGE_BYTES)
        {
            return Err("package byte limit exceeded".into());
        }
        fs::create_dir(directory).map_err(|e| e.to_string())?;
        write_new(
            &directory.join("source-manifest.json"),
            &self.source_manifest,
        )?;
        for (entry, bytes) in manifest.blocks.iter().zip(serialized) {
            write_new(&directory.join(&entry.file), &bytes)?;
        }
        write_new(&directory.join("manifest.json"), &header)
    }
}

fn overlay(bytes: &[u8]) -> Result<(Overlay, Value)> {
    let value = import_json::parse(bytes, MAX_MANIFEST_BYTES)?;
    let format = value["format"].as_str().ok_or("missing QSL format")?;
    if format != QSL_FORMAT {
        return Err(format!(
            "unsupported QSL format: {format}; this bridge imports overlay blocks only"
        ));
    }
    let parsed: Overlay = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if parsed.blocks.is_empty()
        || parsed.blocks.len() > MAX_BLOCKS
        || parsed.patch_storage_dtype != "float32"
        || !valid_hash(&parsed.weights_sha256)
        || !valid_hash(&parsed.artifact_id)
    {
        return Err("invalid QSL manifest or limits".into());
    }
    if parsed.patch_file_bytes > safetensors::MAX_WEIGHT_BYTES {
        return Err("QSL patch byte limit exceeded".into());
    }
    for (key, spec) in &parsed.blocks {
        let layer: usize = key.parse().map_err(|_| "invalid QSL layer index")?;
        if key != &layer.to_string() || layer > 999999 {
            return Err("invalid QSL layer index".into());
        }
        let w = if spec.representation == "svd" {
            spec.original_width
        } else {
            spec.width
        };
        nrl_core::Shape::new(spec.hidden_size, spec.original_width, spec.hidden_size)
            .map_err(|_| "invalid QSL source shape")?;
        nrl_core::Shape::new(spec.hidden_size, w, spec.hidden_size)
            .map_err(|_| "invalid QSL block shape")?;
        if spec.knots < 2
            || spec.knots > 4096
            || w > spec.original_width
            || (spec.representation == "dense" && w != spec.original_width)
        {
            return Err("invalid QSL block specification".into());
        }
        match spec.representation.as_str() {
            "svd" if spec.rank > 0 && spec.rank <= spec.hidden_size.min(w) => (),
            "dense" | "pruned" | "spline-linear" | "spline-cubic" => (),
            other => return Err(format!("unsupported QSL representation or rank: {other}")),
        }
    }
    let fingerprint = parsed.base["fingerprint"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .ok_or("missing bounded base fingerprint")?;
    if artifact_id(fingerprint, &value["blocks"], &parsed.weights_sha256) != parsed.artifact_id {
        return Err("QSL artifact ID mismatch".into());
    }
    let _ = (&parsed.environment, &parsed.details);
    Ok((parsed, value))
}

pub fn import_bytes(manifest: &[u8], weights: &[u8], licence: &str) -> Result<BlockSet> {
    if licence.trim().is_empty() || licence.len() > 4096 {
        return Err("a nonempty bounded weight licence record is required".into());
    }
    if weights.len() > safetensors::MAX_WEIGHT_BYTES {
        return Err("QSL weights byte limit exceeded".into());
    }
    let (source, _) = overlay(manifest)?;
    if source.patch_file_bytes != weights.len() || source.weights_sha256 != hash(weights) {
        return Err("QSL weights checksum or byte count mismatch".into());
    }
    let mut tensors = safetensors::read(weights)?;
    let mut entries = Vec::new();
    let mut blocks = Vec::new();
    let mut unknown = Vec::new();
    let mut expanded = 0usize;
    for (index, (key, spec)) in source.blocks.iter().enumerate() {
        let layer: usize = key.parse().map_err(|_| "invalid QSL layer index")?;
        if key != &layer.to_string() || layer > 999999 {
            return Err("invalid QSL layer index".into());
        }
        let prefix = format!("layer_{layer:03}");
        let n = spec.hidden_size;
        if n == 0 || spec.original_width == 0 || spec.knots < 2 || spec.knots > 4096 {
            return Err("invalid QSL block dimensions or knot count".into());
        }
        let w = if spec.representation == "svd" {
            spec.original_width
        } else {
            spec.width
        };
        nrl_core::Shape::new(n, w, n).map_err(|e| format!("QSL block shape: {e:?}"))?;
        if w > spec.original_width || (spec.representation == "dense" && w != spec.original_width) {
            return Err("invalid QSL retained width".into());
        }
        let mut take = |suffix: &str, shape: &[usize]| -> Result<Vec<f64>> {
            let name = format!("{prefix}.{suffix}");
            let tensor = tensors
                .remove(&name)
                .ok_or_else(|| format!("missing QSL tensor: {name}"))?;
            if tensor.shape != shape {
                return Err(format!("QSL tensor shape mismatch: {name}"));
            }
            Ok(tensor.values)
        };
        let mut block = Artifact {
            schema: "nrl.block/5".into(),
            descriptor: BlockDescriptor {
                kind: "gated-feed-forward/0".into(),
                source_activation: "silu".into(),
                dtype: "f64".into(),
                layout: "row-major".into(),
                state: "stateless".into(),
                parameter_sharing: "independent".into(),
            },
            representation: spec.representation.clone(),
            shape: [n, w, n],
            gate: vec![],
            up: vec![],
            down: vec![],
            spline: None,
            selection: None,
            factors: None,
            bias: None,
        };
        match spec.representation.as_str() {
            "svd" => {
                if spec.rank == 0 || spec.rank > n.min(w) {
                    return Err("invalid QSL SVD rank".into());
                }
                let r = spec.rank;
                block.factors = Some(FactorData {
                    rank: r,
                    gate_left: take("gate_proj.left.weight", &[w, r])?,
                    gate_right: take("gate_proj.right.weight", &[r, n])?,
                    up_left: take("up_proj.left.weight", &[w, r])?,
                    up_right: take("up_proj.right.weight", &[r, n])?,
                    down_left: take("down_proj.left.weight", &[n, r])?,
                    down_right: take("down_proj.right.weight", &[r, w])?,
                });
            }
            "dense" | "pruned" | "spline-linear" | "spline-cubic" => {
                block.gate = take("gate_proj.weight", &[w, n])?;
                block.up = take("up_proj.weight", &[w, n])?;
                block.down = take("down_proj.weight", &[n, w])?;
                if spec.representation.starts_with("spline-") {
                    block.spline = Some(SplineData {
                        knots: spec.knots,
                        low: take("activation.low", &[w])?,
                        high: take("activation.high", &[w])?,
                        values: take("activation.values", &[w, spec.knots])?,
                    });
                }
                if spec.representation != "dense" {
                    block.selection = Some(SelectionData {
                        mapping: "unrecorded".into(),
                        original_channels: spec.original_width,
                        indices: vec![],
                        method: "qsl-unrecorded/0".into(),
                    });
                }
            }
            other => return Err(format!("unsupported QSL representation: {other}")),
        }
        block.validate()?;
        let bytes = serde_json::to_vec_pretty(&block).map_err(|e| e.to_string())?;
        expanded = expanded
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_PACKAGE_BYTES)
            .ok_or("package byte limit exceeded")?;
        let id = format!("qsl.layer.{layer}");
        if block.selection.is_some() {
            unknown.push(id.clone());
        }
        entries.push(Entry {
            id,
            file: format!("block-{index:04}.json"),
            sha256: hash(&bytes),
            bytes: bytes.len(),
        });
        blocks.push(block);
    }
    if !tensors.is_empty() {
        return Err("unused or unsupported QSL tensors".into());
    }
    let manifest_record = Manifest {
        schema: "nrl.block-set/0".into(),
        scope: "standalone-numerical-blocks".into(),
        block_schema: "nrl.block/5".into(),
        source: Source {
            format: source.format,
            manifest_sha256: hash(manifest),
            weights_sha256: source.weights_sha256,
            artifact_id: source.artifact_id,
            original_patch_bytes: weights.len(),
            base: source.base,
            base_identity_verified: false,
            licence: licence.into(),
            numerical_policy: "source-f32-to-f64-exact/0".into(),
        },
        blocks: entries,
        expanded_block_bytes: expanded,
        unrecorded_channel_mappings: unknown,
    };
    Ok(BlockSet {
        manifest: manifest_record,
        blocks,
        source_manifest: manifest.to_vec(),
    })
}

pub fn import_directory(path: &Path, licence: &str) -> Result<BlockSet> {
    let manifest = read_regular(&path.join("manifest.json"), MAX_MANIFEST_BYTES)?;
    overlay(&manifest)?;
    let weights = read_regular(
        &path.join("blocks.safetensors"),
        safetensors::MAX_WEIGHT_BYTES,
    )?;
    import_bytes(&manifest, &weights, licence)
}

pub fn load(path: &Path) -> Result<BlockSet> {
    let bytes = read_regular(&path.join("manifest.json"), MAX_MANIFEST_BYTES)?;
    let value = import_json::parse(&bytes, MAX_MANIFEST_BYTES)?;
    if value["schema"] != "nrl.block-set/0" {
        return Err(format!("unsupported block-set schema: {}", value["schema"]));
    }
    let manifest: Manifest = serde_json::from_value(value).map_err(|e| e.to_string())?;
    if manifest.scope != "standalone-numerical-blocks"
        || manifest.block_schema != "nrl.block/5"
        || manifest.blocks.is_empty()
        || manifest.blocks.len() > MAX_BLOCKS
        || manifest.source.base_identity_verified
        || manifest.source.licence.trim().is_empty()
        || manifest.source.licence.len() > 4096
        || manifest.source.numerical_policy != "source-f32-to-f64-exact/0"
    {
        return Err("unsupported block-set contract".into());
    }
    let source_manifest = read_regular(&path.join("source-manifest.json"), MAX_MANIFEST_BYTES)?;
    if hash(&source_manifest) != manifest.source.manifest_sha256 {
        return Err("source manifest checksum mismatch".into());
    }
    let (original, _) = overlay(&source_manifest)?;
    if original.format != manifest.source.format
        || original.base != manifest.source.base
        || original.artifact_id != manifest.source.artifact_id
        || original.weights_sha256 != manifest.source.weights_sha256
        || original.patch_file_bytes != manifest.source.original_patch_bytes
        || original.blocks.len() != manifest.blocks.len()
    {
        return Err("source provenance mismatch".into());
    }
    let mut total = source_manifest
        .len()
        .checked_add(bytes.len())
        .ok_or("package byte overflow")?;
    let mut expanded = 0usize;
    let mut blocks = Vec::new();
    let mut unknown = Vec::new();
    for (index, (entry, (key, spec))) in manifest.blocks.iter().zip(&original.blocks).enumerate() {
        if entry.id != format!("qsl.layer.{key}")
            || entry.file != format!("block-{index:04}.json")
            || !valid_hash(&entry.sha256)
            || entry.bytes > crate::MAX_ARTIFACT_BYTES
        {
            return Err("invalid block entry or path".into());
        }
        total = total
            .checked_add(entry.bytes)
            .filter(|n| *n <= MAX_PACKAGE_BYTES)
            .ok_or("package byte limit exceeded")?;
        let bytes = read_regular(&path.join(&entry.file), crate::MAX_ARTIFACT_BYTES)?;
        if bytes.len() != entry.bytes || hash(&bytes) != entry.sha256 {
            return Err("block checksum or size mismatch".into());
        }
        expanded += bytes.len();
        let block = crate::parse(&bytes)?;
        if block.representation != spec.representation
            || block.shape
                != [
                    spec.hidden_size,
                    if spec.representation == "svd" {
                        spec.original_width
                    } else {
                        spec.width
                    },
                    spec.hidden_size,
                ]
        {
            return Err("block/source semantics mismatch".into());
        }
        let expect_mapping = matches!(
            spec.representation.as_str(),
            "pruned" | "spline-linear" | "spline-cubic"
        );
        if block.bias.is_some()
            || (spec.representation == "svd"
                && block.factors.as_ref().is_none_or(|f| f.rank != spec.rank))
            || (spec.representation.starts_with("spline-")
                && block.spline.as_ref().is_none_or(|s| s.knots != spec.knots))
            || (expect_mapping
                && block.selection.as_ref().is_none_or(|s| {
                    s.mapping != "unrecorded" || s.original_channels != spec.original_width
                }))
            || (!expect_mapping && block.selection.is_some())
        {
            return Err("imported block/source parameter contract mismatch".into());
        }
        if block
            .selection
            .as_ref()
            .is_some_and(|s| s.mapping == "unrecorded")
        {
            unknown.push(entry.id.clone());
        }
        blocks.push(block);
    }
    if expanded != manifest.expanded_block_bytes || unknown != manifest.unrecorded_channel_mappings
    {
        return Err("block-set size or mapping record mismatch".into());
    }
    Ok(BlockSet {
        manifest,
        blocks,
        source_manifest,
    })
}
