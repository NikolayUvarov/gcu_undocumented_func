use crate::{import_json, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const MAX_WEIGHT_BYTES: usize = 16 << 20;
pub const MAX_HEADER_BYTES: usize = 1 << 20;
pub const MAX_TENSORS: usize = 512;
pub const MAX_VALUES: usize = 262144;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: [usize; 2],
}

pub struct Tensor {
    pub shape: Vec<usize>,
    pub values: Vec<f64>,
}

pub fn read(bytes: &[u8]) -> Result<BTreeMap<String, Tensor>> {
    if bytes.len() < 10 || bytes.len() > MAX_WEIGHT_BYTES {
        return Err("safetensors byte limit or truncated file".into());
    }
    let size = usize::try_from(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
        .map_err(|_| "safetensors header length overflow")?;
    let end = 8usize
        .checked_add(size)
        .filter(|n| *n <= bytes.len())
        .ok_or("truncated safetensors header")?;
    if size > MAX_HEADER_BYTES || bytes[8] != b'{' {
        return Err("invalid or oversized safetensors header".into());
    }
    let mut header = import_json::parse(&bytes[8..end], MAX_HEADER_BYTES)?
        .as_object()
        .ok_or("safetensors header must be an object")?
        .clone();
    if let Some(metadata) = header.remove("__metadata__") {
        let _: BTreeMap<String, String> =
            serde_json::from_value(metadata).map_err(|e| e.to_string())?;
    }
    if header.is_empty() || header.len() > MAX_TENSORS {
        return Err("safetensors tensor count limit".into());
    }
    let data = &bytes[end..];
    let mut ranges = Vec::new();
    let mut total = 0usize;
    let mut tensors = BTreeMap::new();
    for (name, info) in header {
        let info: Header = serde_json::from_value(info).map_err(|e| e.to_string())?;
        if info.dtype != "F32" {
            return Err(format!("unsupported QSL tensor dtype: {}", info.dtype));
        }
        if info.shape.is_empty() || info.shape.len() > 2 || info.shape.contains(&0) {
            return Err("unsupported tensor shape".into());
        }
        let count = info
            .shape
            .iter()
            .try_fold(1usize, |a, b| a.checked_mul(*b))
            .ok_or("tensor shape overflow")?;
        total = total
            .checked_add(count)
            .filter(|n| *n <= MAX_VALUES)
            .ok_or("tensor value limit exceeded")?;
        let [start, stop] = info.data_offsets;
        if stop < start || stop > data.len() || count.checked_mul(4) != Some(stop - start) {
            return Err("invalid safetensors data offsets".into());
        }
        let values: Vec<f64> = data[start..stop]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()) as f64)
            .collect();
        nrl_core::finite(&values).map_err(|_| "nonfinite safetensors value")?;
        ranges.push((start, stop));
        tensors.insert(
            name,
            Tensor {
                shape: info.shape,
                values,
            },
        );
    }
    ranges.sort_unstable();
    let mut cursor = 0;
    for (start, stop) in ranges {
        if start != cursor {
            return Err("safetensors overlapping ranges or holes".into());
        }
        cursor = stop;
    }
    if cursor != data.len() {
        return Err("safetensors trailing data".into());
    }
    Ok(tensors)
}
