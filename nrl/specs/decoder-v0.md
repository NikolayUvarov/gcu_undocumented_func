# Small trained decoder profile v0

**Edition:** 0 (2026-10-26) · **Task:** [900-NRL-0012](../../issues-done/900-NRL-0012-trained-decoder-and-tokenizer.done) · **Scope:** host scalar f64 inference of bounded llama2.c-v0 float32 data. This adds `nrl.decoder/0` independently of the earlier feed-forward graph and block file formats; their limits are unchanged.

## Declared semantics

The adapter `llama2c-f32-v0/0` supports token embeddings, a tied or separate output head, RMSNorm epsilon 1e-5, causal grouped-query attention, stored adjacent-pair RoPE cosine/sine tables, residual addition and bias-free gated SiLU MLPs. Matrix rows are outputs. Source float32 values, including stored positional tables, widen exactly to f64; all arithmetic, normalization, attention/softmax and output logits use scalar f64 (`cpu-scalar-f64-source-f32-stored-rope/0`). Nonfinite values/overflow fail. This differs from the original C float32 arithmetic and its computed RoPE tables; that comparison has a separately stated tolerance. No f32 evidence is transferred to native, GPU or another family.

The primary reader accepts the original seven little-endian signed-i32 header fields, positive dimensions and signed vocabulary/head-sharing flag, followed by the exact original ordered float32 matrices/norms/tables. It checks full shape/length, finite data and absence of trailing bytes. This third-party format is a bridge, not a compatibility reader for old NRL formats. `nrl.decoder-source/0` pins exact model/tokenizer input SHA-256 and carries source/licence/notice declarations. Import checks those declared hashes before parsing; hashes establish identity, not authenticity or a verified licence.

## Limits

The positive dimensions are at most 128, hidden width 512, layers 12, query heads 16, vocabulary 259..1024, context 512. Query/KV head counts divide each other and the dimension; per-head width is even. Source numerical values, including RoPE buffers, are at most 1048576; the complete package is at most 16 MiB, its manifest 1 MiB and tokenizer 1 MiB. The maximum logical KV values `2*context*KV_width*layers` is at most 1048576. Configuration/byte bounds precede decoded numerical allocations.

Prefix input is nonempty, all IDs are in range and sequence length fits context. Forward outputs and optional traces are bounded by 1048576 values. A trace contains every layer's normalized FFN input and actual FFN output; `trace_with_limit` enforces a smaller caller budget before collecting them. A state binds the complete model ID, including every replacement and source declaration. Steps are monotonic, reject a stale/wrong model, bad token or context exhaustion and commit state only after all outputs pass numerical checks. Reset requires a new state. Public steps clone state transactionally, so the logical cache bound is not a process-RSS bound.

## Tokenizer

`tok512.bin` has a little-endian maximum piece length and exact per-token float32 score/u32-length/UTF-8 piece records. Maximum piece length is 128. Duplicate pieces, nonfinite scores, incomplete/trailing data, missing special/space pieces or an incorrect complete 256-byte fallback table are errors. The profile implements the pinned C byte-BPE algorithm: optional BOS, dummy space for nonempty input, UTF-8 codepoint lookup with byte fallback, then the highest-scored adjacent available merge, taking the earliest on a tie; optional EOS follows merging. It performs no Unicode/whitespace normalization and is not an arbitrary SentencePiece adapter.

Text is UTF-8, at most 4096 bytes and cannot contain NUL (the primary C interface's boundary). Raw decoding preserves bytes, strips the dummy leading space after BOS and omits BOS/EOS text. Strict string decoding rejects an incomplete/invalid UTF-8 byte stream. UTF-8/ASCII, combining characters, whitespace and byte fallback have independent C goldens. Generation is deterministic earliest-argmax, at most 64 new tokens, stops on BOS/EOS and checks the full requested context bound first. Probe output includes token IDs, logits, generated IDs, raw decoded bytes and optional valid UTF-8 text; no lossy decoding is hidden.

## Packages and replacements

`nrl.decoder/0` binds adapter/policy, derived configuration, fixed `weights.bin` and `tokenizer.bin` byte counts/hashes, an ordered layer-to-current-artifact table, and complete source declarations. Package/model ID is SHA-256 of typed normalized compact JSON with sorted keys. Loaders use only fixed generated file paths and supplied local data, never source URIs or package code. Unknown versions/fields, paths, mismatched configuration/bytes/hashes and oversized packages fail. Regular-file checks reject symlinks under the declared stable-host-filesystem assumption.

A replacement is a current `nrl.block/5` with exact input/output boundaries and a valid supported operator contract, assigned to an existing layer. It may reduce internal channels, add spline coefficients or use factors. Original source MLPs use the same portable borrowed-block mathematics but their bounded decoder inventory is separate from the small standalone artifact limit. The trained 172-channel source is not silently accepted as a small artifact. Patch files have canonical bytes, fixed names `block-NNNN.json`, and source declarations are retained. Changing a patch invalidates old KV states.

Standalone packages currently retain the original dense weight blob plus replacement files. Information separately reports base/effective trainable counts, effective numeric values (including fixed grids/RoPE), retained numeric values and total package bytes. Fewer effective parameters do not imply smaller disk usage or RSS. The tied head is counted once. No useful-compression claim follows from this representation. Saving requires a new directory/file and writes the manifest last; failed writes may leave an incomplete new output and never remove existing data.

## Evidence and later stages

The pinned real TinyStories-260K/custom tokenizer is MIT-declared by its publisher. Independent vectorized PyTorch f64 gives all-position logits and every FFN input/output for three prompts; unchanged primary C supplies separate float32 logits and nine tokenizer/decoding goldens. Cached versus uncached prefixes, source/state identity, bounds, corrupt input/packages and no-overwrite rules are checked on the NRL branch. The optional generator verifies every primary asset hash and needs the pinned Python environment plus a C compiler; ordinary Rust tests/runtime need neither.

T06 establishes this real dense baseline only. Local calibration/training and optimizer checkpoints are T07; held-out language metrics and comparable timing/memory reports are T08; plans/search and adaptation on student activations are T09/T10. Full Qwen, other tensor/model formats, GPU/quantization, native nnd and general language quality remain unimplemented for this adapter.
