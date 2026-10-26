# Executable feed-forward model packages v0

**Edition:** 0 (2026-10-26) · **Task:** [900-NRL-0010](../../issues/900-NRL-0010-model-graph-and-packages.md) · **Scope:** a complete, bounded, stateless feed-forward model on the host scalar CPU f64 profile.

## Model and adapter contract

`nrl.model/0` describes a whole computation, whereas `nrl.block-set/0` remains a collection of unrelated numerical blocks. This first model profile accepts family `feed-forward`, adapter `nrl.feed-forward`, adapter version 0, preprocessor `identity-f64/0`, stateless execution, and declared tasks `regression-vector/0` and/or `classification-logits/0`. The executor returns raw numerical outputs; quality metrics belong to an evaluator, not the manifest's task label.

The model descriptor contains the graph as its configuration and the exact tied-object table. A future family requires its own adapter/operator contract. No Qwen constants, attention, tokenizer, KV cache, recurrent state, dynamic graph, arbitrary code or universal graph compiler is implied. Unsupported identifiers, versions, operations, dtypes/layouts and backends are refused. Execution schema `nrl.execution/0` accepts only `cpu-scalar`, f64 and row-major layout.

## Graph `nrl.graph/0`

A graph declares a positive fixed input width, a bounded ordered list of uniquely named nodes and one output node. `input` is the reserved input value. Every edge refers to an earlier value; forward/cyclic/missing references are errors. All nodes must contribute to the output. All stored/resolved objects must be used. Names are 1..=64 ASCII letters, digits or `_.-`.

| Operator | Semantics / checks |
|---|---|
| affine | Dense affine object; exact input width; y = W*x+b |
| gated | Current `nrl.block/5`; exact input width; all five representations retain their declared mathematics |
| silu | Elementwise SiLU, unchanged width |
| relu | Elementwise max(x,0), unchanged width |
| add | Elementwise addition of two earlier values with identical widths |

Every computed output is checked for finite values. Add and affine overflow are refusals, not clipping. Rectangular projections are supported. The standalone affine primitive is no_std and allocation-free, with caller-owned output/gradient buffers. Its parameter gradients accumulate and input gradient is replaced; affine data uses `nrl.affine/0`, shape `[inputs,outputs]`, row-major weights and an empty or outputs-length bias vector.

The complete graph currently has a forward executor. A full graph backward/training cycle is not claimed by the affine primitive's backward or by the existing single-block SGD spike.

## Objects, sharing and identity

Objects are explicit IDs mapped to either a dense affine record or a current gated-block artifact. Repeated references to one ID share the same immutable object. The descriptor's tied-object table lists exactly those IDs used by more than one node, sorted by object ID, with node names in graph order. Missing, invented or reordered aliases are refused. Partial tensor aliasing inside different objects is unsupported. Equal bytes in two different IDs still represent two distinct parameter objects and are counted twice.

A model object file contains an adjacent tagged record `{kind,data}`, with kind `affine` or `gated`. Each object manifest entry binds its exact filename, SHA-256, byte length and derived parameter summary (kind/schema, dimensions, representation, width/rank/knots, numeric/trainable/bias counts). Fixed spline grid bounds count in numeric storage but are not trainable values. Existing block bounds still apply.

`model_id` is SHA-256 of the typed manifest normalized to compact UTF-8 JSON: every object key sorted, array order retained, no insignificant whitespace, ordinary JSON string escaping, no Unicode normalization. Manifest numbers are integers; parameter floats are in separately byte-hashed object files. Thus the ID binds graph, parameters, aliasing, adapter, numerical profile, source metadata and change history. Manifest formatting alone does not change it. Identity and byte-integrity checks do not establish authenticity, a signature or a verified licence.

## Standalone and overlay

| Package kind | Required contents and dependency |
|---|---|
| standalone | Every graph object plus the complete descriptor/profile/source/history metadata; no external base |
| overlay | Only changed objects, plus an exact base model ID, matching graph/profile, source records and checked change records; its base must be supplied explicitly |

A loader never follows a base source URI, searches for a base, downloads a missing dependency or executes package code. The supplied base is itself a fully loaded/validated standalone model. Overlay chains are refused in this version. All base object bytes are verified, including objects a patch later replaces. A wrong/missing base, changed graph/profile, unknown object, changed operator kind/input/output boundary, unchanged replacement or inconsistent before/after record is refused.

A replacement may change a gated block's internal width, spline grid or factor rank while preserving its external boundary. Replacing a tied object affects every node that explicitly refers to that object ID. Each change records operation `manual-parameter-replacement/0`, base model ID, before/after object hashes, derived target parameters, caller reason and source identifier. An overlay preserves the base's complete source list and history; new declared source records can be appended.

Exporting a resolved overlay as standalone includes all resolved objects, removes the runtime base dependency and appends the current changes to history. Source licences/notices are retained. No base directory is needed to run this export. Historic hashes are provenance records; retaining history does not implement old format readers or verify an unavailable historical model's authenticity.

Package information separately reports local package bytes, required base bytes, their total, resolved numerical values and aliasing. Overlay size is never the full model's required disk size. Numerical parameter byte counts are logical data sizes, not process RSS: the host also retains parsed objects, immutable source bytes, metadata and buffers. A validated loaded model is an immutable in-memory snapshot; later changes to its source files do not mutate it.

## Limits and file rules

| Bound | Maximum |
|---|---|
| Definition / replacement JSON or covered local package bytes | 16 MiB |
| Manifest | 1 MiB |
| Object file | 1 MiB; current individual numerical limits still apply |
| Objects / graph nodes | 64 each |
| Width | 4096 |
| Total resolved numerical values | 262144 |
| Batch rows | Declared positive limit, at most 4096 |
| Retained graph activation values | Declared positive limit, at most 1048576 |
| Source records / history changes | 32 / 128 |
| Licence / source identifier | 4096 / 2048 UTF-8 bytes |
| Notice texts | Up to 16 per source, 16384 bytes each |
| Output JSON | 32 MiB |

The runtime checks rows times the sum of input/node widths before copying input or allocating activation buffers. This activation budget excludes weights, parsed JSON, per-object numerical scratch and host container overhead. JSON input is byte-bounded and recursion-limited before typed/numerical validation. These host checks are not native memory capabilities, cancellation/deadline enforcement or tested hard-real-time guarantees.

Files are `manifest.json` and generated `object-0000.json`, etc., in sorted local object-ID order. Entry paths must match these exact names before joining. Regular-file checks reject symlinks, under the same stable-host-filesystem assumption as the QSL bridge. Unreferenced directory files are neither read nor included in covered-file totals. A package is data only.

Saving refuses an existing output directory/file. Objects are written before the manifest. A failed write may leave an incomplete new output; no existing user data is removed. Output predictions likewise require a new path. Loaders recheck all covered file hashes/lengths, summaries, graph rules, aliases, provenance dependencies and bounds.

## Source definitions and tools

`nrl.model-definition/0` carries descriptor, execution profile, sources and explicit object data. `nrl.replacements/0` carries one declared source, a reason and replacement object data. Objects are validated before packaging. Source licences/notices are caller declarations, retained without treating NRL's code licence as a model licence. Older/unknown versions are refused by name; no compatibility reader is added.

The host commands are `pack-model`, `check-model`, `run-model`, `export-overlay` and `export-standalone`. Library functions perform the same operations. Predictions have schema `nrl.model-output/0`, exact model ID, row/output widths and values; information has schema `nrl.model-info/0`.

## Evidence and remaining work

Fixtures include an explicitly composed PyTorch graph with shared affine weights, SiLU/ReLU, addition, all five gated representations and a rectangular readout; a second ordinary affine/ReLU/affine network; and separate affine forward/backward tensors. They are synthetic numerical checks, not pretrained-model quality or compression results.

This implements full overlay/standalone semantics for the stated feed-forward profile. T04 still has QSL standalone language models, original-model/tokenizer fingerprint verification and other family contracts; those depend on appropriate model adapters. T05's dataset/task evaluation and training pipeline, T06's language graph and T11's native service remain separate work. The host graph executor does not implement APP's `nnd` or its IDL/authority/session contracts.
