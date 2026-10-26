# QSL block bridge and block-set package v0

**Edition:** 0 (2026-10-26) · **Task:** [900-NRL-0009](../../issues-done/900-NRL-0009-qsl-overlay-block-bridge.done) · **Scope:** numerical blocks, not a whole-model format or executor.

## Source contract

The bridge accepts `qwen-spline-lab-overlay-v1` from the unchanged reference archive. It reads only `manifest.json` and `blocks.safetensors` at the explicitly supplied directory. It never follows `base.source`, downloads files or executes model code. Importing the external QSL reference format does not add compatibility readers for earlier NRL formats. QSL standalone models require graph/adapter work and are refused with the format's name.

Before extracting tensors it checks the format, nonempty bounded blocks, canonical layer indices, dimensions, original/retained width, rank, knot count, declared F32 storage and file length, patch SHA-256 and original QSL artifact ID. The latter hashes sorted-key Python JSON with default separators and Unicode, as `utils.digest` specifies. Fingerprints are retained as declared values; they are not assumed to be hex digests, since the original synthetic demo uses symbolic fingerprints. The complete source manifest is retained byte for byte.

JSON object keys must be unique at every level, including arbitrary source details/environment and safetensors metadata. Typed protocol records reject unknown fields, booleans in integer dimensions and unsupported versions. Strings/data are bounded by the input byte limits. JSON recursion has serde_json's default limit.

## Tensor contract

The F32 safetensors subset has an eight-byte little-endian header length, a JSON tensor map and payload-relative offsets. Each tensor must have a positive one- or two-dimensional shape, exact element/byte length, finite F32 values and the expected QSL name/shape. Offsets must cover the payload contiguously without overlap, holes or trailing data. Metadata values must be strings. Unknown dtypes, missing/unused tensors and unsupported aliases are refused.

Stored F32 parameters and fixed grids are widened exactly to f64; subsequent execution uses NRL's declared CPU f64 profile. This is not an equality claim against the original F32 executor. The external fixture reloads the saved QSL patch and widens its parameters before generating the comparison outputs.

The bridge handles dense/pruned SiLU, linear and cubic Hermite splines, and all six SVD factors. Source shapes are `[hidden_size, retained_width, hidden_size]`, with original width retained for SVD. No full Qwen graph, attention, tokenizer or KV state is constructed. Source block names become explicit adapter-local IDs such as `qsl.layer.2`; the numerical core has no model-family name.

QSL does not record retained source-channel indices. Imported pruned/spline blocks therefore use schema 5's `mapping=unrecorded`, an empty index array and `method=qsl-unrecorded/0`. Original width is retained. The importer does not infer indices from weights that may have been trained. Dense and SVD need no retained-channel map. The source overlay is bias-free; unexpected bias tensors are refused.

## Resource bounds

| Bound | Maximum |
|---|---|
| Input manifest / safetensors JSON header | 1 MiB each |
| Source weight file | 16 MiB |
| Blocks | 32 |
| Tensors | 512 |
| Total imported F32 values | 262144 |
| Source knot count | 2..=4096 |
| Layer index | Canonical nonnegative decimal, at most 999999 |
| Individual runtime block | Existing schema 5 limits, including 16384 values |
| Covered package files together | 16 MiB |
| Caller-supplied licence record | 1..=4096 UTF-8 bytes, not whitespace-only |

Lengths and products are checked before slicing or allocation based on the claimed sizes. The host adapter rejects symlinked input files and uses fixed/generated filenames. These checks assume a stable host filesystem; no race-resistant OS sandbox or native authority enforcement is claimed. Core/representation libraries remain allocation-free and no_std; this importer is host-only std.

## `nrl.block-set/0`

The package has `manifest.json`, `source-manifest.json`, and generated files `block-0000.json`, etc. Each block is a complete `nrl.block/5` artifact. The package manifest contains:

- `schema`, exactly `nrl.block-set/0`; `block_schema`, exactly `nrl.block/5`;
- `scope`, exactly `standalone-numerical-blocks`;
- `source`: original format, raw manifest SHA-256, patch SHA-256 and byte length, original artifact ID, complete declared base metadata, `base_identity_verified=false`, caller-provided weight licence and `source-f32-to-f64-exact/0` numerical policy;
- `blocks`: a bounded ordered list of ID, exact generated filename, SHA-256 and byte length;
- `expanded_block_bytes` and explicit `unrecorded_channel_mappings`.

The loader checks source-manifest bytes/hash, QSL artifact-ID consistency, block IDs and generated filenames, every block's bytes/hash, exact source representation/shape/rank/grid/mapping contract, and size/count records. No manifest-supplied relative path is joined unchecked. Extra directory files are not read or included in covered-file byte totals.

All parameters required to execute **these blocks** are included; original model files are unnecessary for their standalone numerical evaluation. Running the whole model still requires its graph, unchanged weights, tokenizer/preprocessing and adapter. `base_identity_verified=false` is mandatory: the bridge has not hashed or validated those external objects. A small patch or block-set size is never reported as full-model size.

SHA-256 checks establish self-consistency against the supplied records, not source authenticity, a signature or licence verification. The licence text is provided by the importer caller and preserved, rather than replaced by NRL's code licence. Import itself does not publish or upload anything.

Saving refuses an existing output directory and existing output files. Blocks/source are written before the manifest. A write error can leave an incomplete new directory; it is not a successful package and must be validated again. The writer does not delete or overwrite user data. Loading and resaving preserve numerical parameters/outputs and the raw original source manifest; package JSON may be normalized by its writer.

## Remaining T04

This bridge does not complete general ModelDescriptor/graph contracts, source-model fingerprint verification, QSL standalone import, full-model overlay/standalone export, model licences/notice collection, calibration lineage or model adapters. Those remain separate tasks under 900. The package is scoped so a numerical block collection cannot be mistaken for a runnable language model.
