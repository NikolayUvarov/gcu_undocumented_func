use crate::{import_json, storage, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
pub mod data;
pub mod local;
pub use data::{Calibration, Corpus, Document};
pub use local::{LocalConfig, LocalState, Transform};
pub(crate) const MAX_BYTES: usize = 16 << 20;
pub(crate) fn parse<T: DeserializeOwned>(bytes: &[u8], schema: &str) -> Result<T> {
    let v = import_json::parse(bytes, MAX_BYTES)?;
    if v["schema"].as_str() != Some(schema) {
        return Err(format!(
            "unsupported research schema: {}; expected {schema}",
            v["schema"]
        ));
    }
    serde_json::from_value(v).map_err(|e| e.to_string())
}
pub(crate) fn bytes<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let b = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    if b.len() > MAX_BYTES {
        return Err("research JSON byte limit".into());
    }
    Ok(b)
}
pub(crate) fn id<T: Serialize>(v: &T) -> Result<String> {
    Ok(storage::hash(
        import_json::canonical_compact(&serde_json::to_value(v).map_err(|e| e.to_string())?)
            .as_bytes(),
    ))
}
pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    storage::read_regular(path, MAX_BYTES)
}
pub(crate) fn save<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    storage::write_new(path, &bytes(v)?)
}
pub(crate) fn finite(v: &[f64]) -> Result<()> {
    nrl_core::finite(v).map_err(|_| "nonfinite research value".into())
}
pub(crate) fn name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
