//! Complete supported-decoder binding for the external QSL overlay format.
use crate::{decoder::Decoder, model::Source, qsl, research, storage, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: String,
    pub adapter: String,
    pub base_model_id: String,
    pub qsl_artifact_id: String,
    pub layers: BTreeMap<usize, usize>,
    pub source: Source,
}
impl Binding {
    pub fn load(path: &Path) -> Result<Self> {
        research::parse(&research::read(path)?, "nrl.qsl-decoder-binding/0")
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    binding: Binding,
    source_manifest_sha256: String,
    source_manifest_bytes: usize,
    weights_sha256: String,
    weights_bytes: usize,
    licence: String,
    resolved_model_id: String,
}
pub struct Import {
    pub model: Decoder,
    manifest: Manifest,
    source_manifest: Vec<u8>,
    weights: Vec<u8>,
}
fn bind(
    base: &Decoder,
    manifest_bytes: Vec<u8>,
    weights: Vec<u8>,
    binding: Binding,
    licence: &str,
) -> Result<Import> {
    let blocks = qsl::import_bytes(&manifest_bytes, &weights, licence)?;
    crate::model::validate_sources(std::slice::from_ref(&binding.source))?;
    let source = &blocks.manifest().source;
    if binding.schema != "nrl.qsl-decoder-binding/0"
        || binding.adapter != "llama2c-f32-v0/0"
        || !base.patches().is_empty()
        || binding.base_model_id != base.model_id()
        || binding.qsl_artifact_id != source.artifact_id
        || source.base["fingerprint_policy"] != "nrl-decoder-model-id/0"
        || source.base["fingerprint"] != base.model_id()
        || source.base["model_type"] != "llama2c-f32-v0/0"
        || binding.layers.len() != blocks.manifest().blocks.len()
        || binding.layers.is_empty()
        || binding.layers.len() > base.config().layers
        || binding.layers.values().collect::<BTreeSet<_>>().len() != binding.layers.len()
        || binding
            .layers
            .iter()
            .any(|(a, b)| a != b || *b >= base.config().layers)
    {
        return Err("QSL complete model requires exact dense base/adapter/fingerprint/layer bindings; HF/Qwen fingerprints are unsupported".into());
    }
    let mut patches = BTreeMap::new();
    let source_value: serde_json::Value =
        crate::import_json::parse(&manifest_bytes, qsl::MAX_MANIFEST_BYTES)?;
    for (key, a) in blocks.blocks() {
        let layer: usize = key
            .strip_prefix("qsl.layer.")
            .ok_or("invalid QSL layer key")?
            .parse()
            .map_err(|_| "invalid QSL layer index")?;
        let target = *binding
            .layers
            .get(&layer)
            .ok_or("missing explicit QSL layer binding")?;
        let dense = base.dense(target)?;
        if a.shape[0] != dense.dim
            || a.shape[2] != dense.dim
            || source_value["blocks"][layer.to_string()]["original_width"].as_u64()
                != Some(dense.hidden as u64)
        {
            return Err("QSL replacement differs from full model FFN geometry".into());
        }
        patches.insert(target, a.clone());
    }
    let notice =
        String::from_utf8(manifest_bytes.clone()).map_err(|_| "QSL manifest must be UTF-8")?;
    if notice.len() > 16384 {
        return Err("complete QSL source notice byte limit".into());
    }
    let provenance=Source {identifier:format!("qsl-overlay://{}",source.artifact_id),sha256:Some(source.manifest_sha256.clone()),licence:licence.into(),notices:vec!["External QSL overlay; source F32 parameters exactly widened to scalar F64. Base fingerprint verified under explicit NRL model-ID policy, not Hugging Face model_fingerprint.".into(),notice,format!("Binding: {}",serde_json::to_string(&binding).map_err(|e|e.to_string())?)]};
    let model = base.with_patches(patches, &[binding.source.clone(), provenance])?;
    let manifest = Manifest {
        schema: "nrl.qsl-decoder-overlay/0".into(),
        binding,
        source_manifest_sha256: storage::hash(&manifest_bytes),
        source_manifest_bytes: manifest_bytes.len(),
        weights_sha256: storage::hash(&weights),
        weights_bytes: weights.len(),
        licence: licence.into(),
        resolved_model_id: model.model_id().into(),
    };
    Ok(Import {
        model,
        manifest,
        source_manifest: manifest_bytes,
        weights,
    })
}
pub fn import(base: &Decoder, source: &Path, binding: Binding, licence: &str) -> Result<Import> {
    let manifest = storage::read_regular(&source.join("manifest.json"), qsl::MAX_MANIFEST_BYTES)?;
    let weights = storage::read_regular(
        &source.join("blocks.safetensors"),
        crate::safetensors::MAX_WEIGHT_BYTES,
    )?;
    bind(base, manifest, weights, binding, licence)
}
impl Import {
    pub fn save_overlay(&self, path: &Path) -> Result<()> {
        fs::create_dir(path).map_err(|e| e.to_string())?;
        fs::create_dir(path.join("qsl")).map_err(|e| e.to_string())?;
        storage::write_new(&path.join("qsl/manifest.json"), &self.source_manifest)?;
        storage::write_new(&path.join("qsl/blocks.safetensors"), &self.weights)?;
        research::save(&path.join("manifest.json"), &self.manifest)
    }
    pub fn info(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"schema":"nrl.qsl-decoder-info/0","base_model_id":self.manifest.binding.base_model_id,"model_id":self.model.model_id(),"qsl_artifact_id":self.manifest.binding.qsl_artifact_id,"base_identity_verified":true,"fingerprint_policy":"nrl-decoder-model-id/0","binding":self.manifest.binding,"overlay_bytes":self.source_manifest.len()+self.weights.len()+research::bytes(&self.manifest)?.len(),"complete_model":self.model.info()?,"standalone_export":"nrl.decoder/0, including unchanged source weights/tokenizer and replacement blocks","scope":"complete supported llama2c adapter; not a Qwen/HF standalone importer","native_execution":false}),
        )
    }
}
pub fn load(base: &Decoder, path: &Path) -> Result<Import> {
    let raw = research::read(&path.join("manifest.json"))?;
    let saved: Manifest = research::parse(&raw, "nrl.qsl-decoder-overlay/0")?;
    if raw != research::bytes(&saved)?
        || saved.source_manifest_bytes > qsl::MAX_MANIFEST_BYTES
        || saved.weights_bytes > crate::safetensors::MAX_WEIGHT_BYTES
    {
        return Err("QSL overlay canonical manifest/size mismatch".into());
    }
    let source = storage::read_regular(&path.join("qsl/manifest.json"), qsl::MAX_MANIFEST_BYTES)?;
    let weights = storage::read_regular(
        &path.join("qsl/blocks.safetensors"),
        crate::safetensors::MAX_WEIGHT_BYTES,
    )?;
    if source.len() != saved.source_manifest_bytes
        || weights.len() != saved.weights_bytes
        || storage::hash(&source) != saved.source_manifest_sha256
        || storage::hash(&weights) != saved.weights_sha256
    {
        return Err("QSL overlay input checksum/size mismatch".into());
    }
    let result = bind(base, source, weights, saved.binding.clone(), &saved.licence)?;
    if research::id(&result.manifest)? != research::id(&saved)? {
        return Err("QSL resolved model/provenance differs from overlay".into());
    }
    Ok(result)
}
