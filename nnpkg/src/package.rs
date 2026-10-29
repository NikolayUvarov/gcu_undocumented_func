//! Loading a standalone `nrl.model/0` package (262-APP-0112, docs/nn/runtime.md section 2): the model ID from the
//! manifest's canonical form, each object file by its length and SHA-256, its record re-encoded in the writer profile
//! and compared byte for byte, then the checks of `nrl-lab`'s reader and `nrl-runtime`'s layout.
use crate::check::{self, count_bound, MAX_ARTIFACT_BYTES, MAX_HISTORY, MAX_MANIFEST_BYTES, MAX_MODEL_BYTES, MAX_OBJECTS};
use crate::model::{self, Manifest, ObjectData, Summary};
use crate::program::Prepared;
use crate::{encode, json, sha256, Refusal};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// A checked package: its manifest, its objects sorted by key, its ID, the bytes of its files, and the runtime's
/// view of it.
pub struct Package {
    pub manifest: Manifest,
    pub objects: Vec<(String, ObjectData)>,
    pub id: [u8; 32],
    pub bytes: usize,
    /// The runtime's position of the graph's output (a node's index + 1).
    pub output: usize,
    pub input_width: usize,
    pub output_width: usize,
    /// Values a row's activations take, and with the largest scratch, `nrl-runtime`'s workspace.
    pub values_per_row: usize,
    pub workspace_values: usize,
    pub work_per_row: u64,
}

impl Package {
    pub fn object(&self, key: &str) -> Option<&ObjectData> { self.objects.iter().find(|(k, _)| k == key).map(|(_, d)| d) }
    /// Rows a job may hold, and the values its activations may take (the execution profile).
    pub fn rows_max(&self) -> usize { self.manifest.execution.max_batch_rows }
    pub fn activations_max(&self) -> usize { self.manifest.execution.max_activation_values }
    /// What nnd holds of it: its files' bytes.
    pub fn held_bytes(&self) -> u64 { self.bytes as u64 }
}

fn hex(digest: &[u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0; 64];
    for (i, b) in digest.iter().enumerate() { out[2 * i] = HEX[(b >> 4) as usize]; out[2 * i + 1] = HEX[(b & 15) as usize]; }
    out
}

/// The model ID of a manifest: the SHA-256 of its canonical form.
pub fn model_id(manifest: &Manifest) -> [u8; 32] {
    let mut bytes = Vec::new();
    encode::canonical(&mut bytes, &encode::manifest(manifest));
    sha256::digest(&bytes)
}

/// The manifest file read and its ID checked (steps 2 and 3).
pub fn manifest(bytes: &[u8], id: &[u8; 32]) -> Result<Manifest, Refusal> {
    if bytes.len() > MAX_MANIFEST_BYTES { return Err(Refusal::TooLarge("A MANIFEST OF MORE THAN 1 MiB")); }
    let value = json::parse(bytes).map_err(Refusal::Malformed)?;
    let m = model::manifest(&value)?;
    if model_id(&m) != *id { return Err(Refusal::Digest("THE MODEL ID IS NOT THE ONE GIVEN")); }
    if m.objects.is_empty() || m.objects.len() > MAX_OBJECTS { return Err(Refusal::TooLarge("NO OBJECT, OR MORE THAN 64")); }
    match m.kind.as_str() {
        "standalone" if m.base.is_none() && m.changes.is_empty() => Ok(m),
        "standalone" => Err(Refusal::Malformed("A STANDALONE MODEL WITH A BASE OR CHANGES")),
        "overlay" => Err(Refusal::Unsupported("AN OVERLAY: VERSION 1.0 LOADS STANDALONE PACKAGES ONLY")),
        _ => Err(Refusal::Unsupported("A PACKAGE KIND OTHER THAN standalone")),
    }
}

/// The file name the manifest must give the object at `index` (in key order).
pub fn object_file(index: usize) -> String { format!("object-{index:04}.json") }

/// One object file against its manifest entry (steps 4 and 5): its record and summary.
pub fn object(raw: &[u8], record: &model::ObjectRecord) -> Result<(ObjectData, Summary), Refusal> {
    if raw.len() != record.bytes || hex(&sha256::digest(raw)) != record.sha256.as_bytes() { return Err(Refusal::Digest("AN OBJECT FILE'S LENGTH OR SHA-256 IS NOT ITS ENTRY'S")); }
    let data = model::object(&json::parse(raw).map_err(Refusal::Malformed)?)?;
    let summary = check::summary(&data)?;
    if summary != record.parameters { return Err(Refusal::Malformed("AN OBJECT'S SUMMARY IS NOT ITS ENTRY'S")); }
    if encode::object_bytes(&data) != raw { return Err(Refusal::Malformed("AN OBJECT FILE NOT IN THE WRITER PROFILE nrl.serde-json-pretty-object/0")); }
    Ok((data, summary))
}

/// The package whose manifest file is `manifest_bytes`, which must have model ID `id`; `file(name)` reads an object
/// file of it (None: absent).
pub fn load<'f>(manifest_bytes: &[u8], id: &[u8; 32], mut file: impl FnMut(&str) -> Option<&'f [u8]>) -> Result<Package, Refusal> {
    let m = manifest(manifest_bytes, id)?;
    let mut total = manifest_bytes.len();
    let mut objects = Vec::new();
    let mut summaries = Vec::new();
    for (index, (key, entry)) in m.objects.iter().enumerate() {
        if !check::id(key) || entry.file != object_file(index) || !check::valid_hash(&entry.sha256) || entry.bytes > MAX_ARTIFACT_BYTES {
            return Err(Refusal::Malformed("AN OBJECT'S PATH OR RECORD NOT VALID"));
        }
        total = count_bound(total, entry.bytes, MAX_MODEL_BYTES)?;
        let raw = file(&entry.file).ok_or(Refusal::NotFound("AN OBJECT FILE OF THE PACKAGE IS MISSING"))?;
        let (data, summary) = object(raw, entry)?;
        objects.push((key.clone(), data));
        summaries.push((key.clone(), summary));
    }
    let graph = check::graph(&m.descriptor, &m.execution, &summaries)?;
    check::sources(&m.sources)?;
    check::history(&m.history)?;
    check::history(&m.changes)?;
    if m.history.len() + m.changes.len() > MAX_HISTORY { return Err(Refusal::TooLarge("MORE THAN 128 CHANGES")); }
    let mut pretty = Vec::new();
    encode::pretty(&mut pretty, &encode::manifest(&m), 0);
    if pretty.len() > MAX_MANIFEST_BYTES { return Err(Refusal::TooLarge("A MANIFEST OF MORE THAN 1 MiB")); }
    let mut package_bytes = pretty.len();
    for (_, entry) in &m.objects { package_bytes = count_bound(package_bytes, entry.bytes, MAX_MODEL_BYTES)?; }
    let output_width = graph.widths[graph.output];
    let mut package = Package { manifest: m, objects, id: *id, bytes: total, output: graph.output, input_width: graph.widths[0], output_width, values_per_row: 0, workspace_values: 0, work_per_row: 0 };
    let layout = {
        let prepared = Prepared::new(&package)?;
        let nodes = prepared.operations()?;
        let program = prepared.program(&nodes)?;
        program.layout().map_err(|_| Refusal::Unsupported("THE RUNTIME'S PROFILE REFUSES THE GRAPH"))?
    };
    package.values_per_row = layout.values_per_row;
    package.workspace_values = layout.values_per_row + layout.scratch_values;
    package.work_per_row = layout.work_units_per_row;
    Ok(package)
}
