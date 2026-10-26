# Neural Representation Lab: Rust model research workspace

NRL is track `NRL`, issue [900](../issues/900-neural-representation-lab.md). Host adapters cover bounded feed-forward graphs, a real trained TinyStories decoder and a mean-token embedding family. Five gated SiLU block representations (`dense`, `pruned`, `spline-linear`, `spline-cubic`, `svd`) share checked affine/gradient mathematics. Model packages, calibration/training, three task profiles, measured plan/recovery experiments and a portable graph/session boundary are implemented. Native MIND Core service integration remains T11.

## Current branch status (2026-10-26)

T06–T10 are complete for the declared bounded host profiles. The latest branch has **82 host tests**, four bare-metal rlib builds (`nrl-repr` and `nrl-runtime`, both targets) and all CLI cycles passing. Run `bash nrl/tools/check.sh` from the repository root to repeat them. Code landing/full repository gating remains pending.

| Work | Current scope and outcome | Evidence |
|---|---|---|
| T06–T10 | Real trained decoder/tokenizer, resumable calibration/local training, two-task metrics, mixed-plan search and student-input recovery | [Recovery and previous work](reports/recovery-2026-10-26.json) |
| T11 — partial | Portable no_std/no_alloc graph executor, owner/model-bound session state, cumulative budgets, cooperative deadline/cancel controls and verified host bridge | [Host preparation](reports/native-boundary-2026-10-26.json) |
| T12 — first host experiment | Immutable FFN views/scratch reused per document; full forward promoted after fixed paired measurements; no numerical/quality change | [Pre/post measurements](reports/optimization-2026-10-26.json), [final build](reports/optimization-final-build-2026-10-26.json) |
| T13 — first host extension | Independent mean-token embedding family, shared five-path projection, standalone export/pruning, Recall/MRR/NDCG and shared evaluation/benchmark/compare | [Independent reference and example](reports/embedding-2026-10-26.json) |

The prepared decoder's recorded paired medians are about 5.8–5.9 ms for the original path and 4.1 ms for preparation (about 30% less elapsed time), with zero observed logit/argmax differences and fixed quality limits met. Each report records its executable/host/workload; these are scalar host observations, not native/device/generation guarantees. Source weights and numerical policy are unchanged. The original checked path remains available.

The compression study still recommends dense: recovered-student held-out perplexity is about 105.05 versus dense 9.36 on the miniature corpus. T12 improves execution of the same model without changing its weights. The embedding/retrieval fixture is original synthetic data and does not establish representative semantic quality, pretrained quality or accepted compression benefit.

**T11 remains open.** APP owns the absent `nnd`/`idl/nn.wit`; the [request](../issues/requests-APP.md#nrl-native-nnd-integration-for-900-t11) records native parser/client authentication/memory/time/worker/cancellation and named-target test requirements. The portable graph profile does not cover native decoder KV or native embedding execution. Cross-builds and logical host sessions are not OS isolation/native execution evidence. Further T12 SIMD/parallel/device work needs its target measurements; broader T13 families and full embedding lookup/pooling/normalization training remain separate extensions. Parent 900 stays open; deferred landing does not block independent host work.

## Run from the repository root

```sh
cargo test --manifest-path nrl/Cargo.toml --locked
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target x86_64-unknown-none
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target aarch64-unknown-none-softfloat
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- demo nrl/tests/fixtures/f64-v5.json /tmp/nrl-demo-new
```

The output directory must not exist. The demo checks reference outputs and gradients, takes one SGD step in each fixture and checks a block-file round trip. It writes twelve block files and a JSON report. Errors use a nonzero exit status; existing files are never overwritten. A failed I/O operation can leave incomplete new outputs.

Use `--offline` as well when the locked dependencies are already cached. `nrl/tools/check.sh` runs all tests, portable builds and complete fresh import/train/measure/search/recovery/check/replay cycles. Python is not required to run the Rust tests or demonstration.

The project's pinned nightly builds the code. If that toolchain lacks rustfmt, `cargo +stable fmt --manifest-path nrl/Cargo.toml --all --check` checks formatting with an installed stable formatter; it does not select the compiler used by the tests.

## Structure

| Component | Responsibility |
|---|---|
| `crates/nrl-core` | no_std shapes, limits, errors |
| `crates/nrl-repr` | no_std, allocation-free math over explicit borrowed buffers; libm exponential |
| `crates/nrl-runtime` | no_std/no_alloc graph/session executor, caller-owned buffers, cumulative budgets and cooperative controls; native service glue remains pending |
| `crates/nrl-lab` | std files, a bounded block artifact, batch loss, a single SGD step, demo/report |
| `specs/block-v5.md` | numerical, format, ownership and refusal contracts |
| `tests/fixtures/f64-v5.json` | externally generated reference tensors, no trained weights |
| `tools/` | explicit reference validation, fixture generation and Rust checking |
| `reports/` | dated evidence with its environment and limits |
| `reference/` | unchanged original prototype and a pinned reproduction environment |

[Inventory and Python reproduction](../docs/nrl/reference-inventory.md), [backend ADR](../docs/nrl/ADR-0001-cpu-backend.md), [block contract](specs/block-v5.md), [source attribution](NOTICE).

## Evidence and boundaries

The recorded Python run is **64 passed, zero skipped**, including a random tiny Qwen graph; it is not a trained-Qwen quality result. The first Rust tests compare stored f64 tensors and independent numerical gradients, including knot and tail conventions and malformed inputs. Dated reports state the actual run, not future support.

A cross-compiled library is not a native application run. APP owns issue 262's `nnd`, its IDL and model sessions; the first Rust workspace changes none of those. KRN/DRV own the platform and accelerator changes. No OS profile guarantee is added by these host tests.

Remaining: full-model contracts, QSL import, full-model artifacts, independent model families, full calibration/training/evaluation/search, CPU optimization, native integration and GPU work. The first block round trip is only a small part of T04. No compression or speed benefit on a real trained network is established.

CI scripts do not yet invoke this new workspace; the owning KRN track has a request to add the explicit checks. Until it lands, run them manually and include their result in the landing request.

## Pruned control (900-NRL-0005)

`nrl_lab::prune` ranks source channels by weight energy or by training/calibration activations, with stable ties, and records the retained indices. `linear_spline_control` uses exactly those indices and projection weights. Scoring is explicitly f64; Python's ranking uses f32. This is a block transform, not a full-model conversion or a quality result. Artifact `nrl.block/5` replaces `/0` through `/4`, which are refused by name. Recreate older prototype files; there is no compatibility reader.

## Cubic Hermite (900-NRL-0006)

`spline-cubic` ports the reference's Hermite segments, neighbour-dependent coefficient gradients and linear tails. It is C1 at the knots and reduces to linear interpolation for two knots. It is not a B-spline and can overshoot. `cubic_spline_control` retains the pruned control's exact projection weights and channel mapping. All external fixtures now use block schema 5.

## SVD factors (900-NRL-0007)

`svd` runs six factor matrices directly, with analytic gradients and caller-owned scratch. Diagnostic reconstruction is separate. `nrl_lab::svd::control` initializes a dense block with a small f64 Jacobi solver; each matrix dimension is limited to 64 and nonconvergence is refused. It is not a large-model solver, SVD-LLM or a measured optimization. All five representations now preserve optional output biases; unsupported source semantics are refused.

## Latest verified branch slice (2026-10-26)

900-NRL-0005 through 0007 are complete on the NRL branch: **25 host tests**, release rlibs for both portable targets, and differential forward/backward, one SGD step and exact round trips for all five representations. The nine small initializer cases also compare singular values/reconstruction/truncation with an external f64 SVD oracle. [Current report](reports/svd-f64-2026-10-26.json), [raw check log](reports/svd-check-2026-10-26.txt). This is source-commit-specific host evidence; no native run or real-model benefit is established.

Next: general QSL/model artifact contracts and import/export (T04), then an independent small feed-forward model (T05). Landing is asynchronous integration, not a host-development prerequisite.

## Affine extension and applicability (900-NRL-0008)

All five paths support optional independent output biases on gate/up/down, including mixed absent/present biases and analytic gradients. Packing selects gate/up biases and preserves the down bias; SVD leaves each output bias after L*R. The /1 ranking IDs specify affine energy heuristics. Schema 5 requires an explicit semantic descriptor. Only stateless, independent-parameter, row-major f64 gated SiLU sources on the scalar CPU profile are accepted; `block_on` refuses other devices without fallback. [Capability matrix](specs/block-v5.md#descriptor-and-capability-matrix).

The fixture includes five unchanged QSL graph cases, five affine extensions and two rectangular affine cases checked through PyTorch. Those extensions are not capabilities of the original archive. Schema 0–4 files are refused by name before reading the new required fields; regenerate them rather than adding a compatibility reader.

## Affine profile verified (2026-10-26)

900-NRL-0008 passed **31 host tests**, both portable release rlibs and **12 differential/train/round-trip cases**, including every bias/factor gradient and rectangular blocks. [Report](reports/affine-f64-2026-10-26.json), [raw log](reports/affine-check-2026-10-26.txt). The five-representation gated SiLU numerical profile is complete at these bounds; full-model contracts and T04 onward remain open. This is host evidence only.

## QSL overlay bridge (900-NRL-0009)

```sh
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- import-qsl nrl/tests/fixtures/qsl-overlay-v1 /tmp/nrl-qsl-new "MIT; deterministic synthetic fixture"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-block-set /tmp/nrl-qsl-new
```

The first command verifies and imports all five QSL overlay block representations without Python or a network call, exactly widening saved F32 parameters to f64. The new output directory must not exist; the weight licence record is required. It preserves the original manifest, patch/source hashes, explicit unverified base identity and unrecorded channel mappings. `nrl.block-set/0` is a standalone set of numerical blocks, not a standalone Qwen model or the rest of its graph. QSL standalone model files are explicitly refused. [Bridge and block-set contract](specs/qsl-block-bridge-v0.md).

Limits: 32 blocks, 512 tensors, 262144 imported values, 1 MiB JSON/header, 16 MiB source weights and covered package files; individual block limits still apply. There is no hidden Python, download, dtype fallback or source-path traversal. Fixed regular-file names are used; this host filesystem check is not a tested adversarial-filesystem or native capability boundary. Failed output writes can leave an incomplete new directory; the manifest is written last.

`tools/generate_qsl_fixture.py` replays the verified original QSL writer for the tiny synthetic overlay. With `--check`, tensor bytes, external f64 outputs and the core manifest must match; the recorded writer environment is excluded from cross-host regeneration comparison. No pretrained weights are included.

## Import bridge verified (2026-10-26)

900-NRL-0009 passed **37 host tests**, both portable rlibs, the 12-case numerical profile and the five-representation QSL import/package/CLI cycle. Tensor bytes and outputs regenerate through the original writer; its environment record is kept separately from the core-manifest reproducibility check. [Current report](reports/qsl-block-bridge-2026-10-26.json), [raw log](reports/qsl-check-2026-10-26.txt). Next is a model/graph descriptor and complete overlay/standalone semantics, followed by the independent small model; the block bridge does not close T04.

## Complete feed-forward models (900-NRL-0010)

`nrl.model/0` adds an executable stateless graph with dense affine layers, gated blocks, SiLU/ReLU and addition, plus explicit shared object IDs and bounded batches/activations. A standalone package includes all graph weights. An overlay includes only changed objects and requires the exact explicitly supplied standalone base; export resolves it into a standalone package with licences/notices and change history retained. [Graph/package contract](specs/model-v0.md).

```sh
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- pack-model nrl/tests/fixtures/model-v0/definition.json /tmp/nrl-model-base-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-overlay /tmp/nrl-model-base-new nrl/tests/fixtures/model-v0/replacements.json /tmp/nrl-model-overlay-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-model /tmp/nrl-model-overlay-new nrl/tests/fixtures/model-v0/input.json /tmp/nrl-model-output-new.json /tmp/nrl-model-base-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-standalone /tmp/nrl-model-overlay-new /tmp/nrl-model-flat-new /tmp/nrl-model-base-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-model /tmp/nrl-model-flat-new
```

Every output path above must be new. `model-v0/plain.json` is an independent ordinary affine/ReLU/affine model. The graph supports the separate full-graph regression training/evaluation profile below; classification, QSL whole-language-model import and native execution remain separate work. Parameter/file counts are reported with the required base dependency; they are not measured memory/performance or useful-compression claims.

900-NRL-0010 passed **44 host tests**, both portable rlibs, complete external graph/affine-gradient checks and the base/overlay/standalone CLI cycle. All six model fixtures regenerate byte-identically. [Dated model report](reports/model-graph-2026-10-26.json), [raw log](reports/model-check-2026-10-26.txt). The exported standalone runs without its base files; CLI metadata identifies the written artifact. Next is T05's task/data/training pipeline; these synthetic fixtures establish no model quality or optimization result.


## Full-graph regression experiments (900-NRL-0011)

The independent `nrl.feed-forward` adapter now supports full-graph gradients and deterministic full-batch SGD, including shared objects, all five gated representations, affine/activation/add operators and explicit logical training-buffer/work limits. [Data, selection, provenance and limits](specs/regression-v0.md).

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- pack-model nrl/tests/fixtures/model-v0/plain.json /tmp/nrl-regression-initial-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- train-regression /tmp/nrl-regression-initial-new nrl/tests/fixtures/training-v0/data.json nrl/tests/fixtures/training-v0/run.json /tmp/nrl-regression-run-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-experiment /tmp/nrl-regression-run-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- replay-regression /tmp/nrl-regression-run-new /tmp/nrl-regression-replay-new
```

All output paths must be new. Data/config specify their sources, split membership, fixed row order and budgets. Train supplies gradients; validation chooses the earliest best MSE checkpoint, including epoch zero; test metrics are computed only after selection. The selected standalone model binds the saved training record and retains source licences/notices and existing history. Replay restarts the full experiment from the saved initial model/data/config; it is not optimizer resume. Reproducibility is checked on the recorded host configuration, not promised across arbitrary platforms or toolchains. The synthetic regression fixture is a numerical/task experiment, not a pretrained-model quality or optimization result.

Optional oracle regeneration uses the pinned reference environment (Rust checks and runtime need no Python):

```bash
/path/to/reference-venv/bin/python nrl/tools/generate_training_fixtures.py --reference-root /path/to/qwen-spline-lab --output nrl/tests/fixtures/training-v0 --check
```

900-NRL-0011 passed **52 host tests**, both portable rlibs, external full-graph gradients/SGD/history checks and complete train/check/replay CLI cycles. [Dated evidence](reports/regression-training-2026-10-26.json), [raw log](reports/training-check-2026-10-26.txt), [saved experiment](reports/regression-example-2026-10-26/report.json). Its actual Rust history selects epoch 91 of 128. The example retains both standalone models and complete data/config/records; replay it with:

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- replay-regression nrl/reports/regression-example-2026-10-26 /tmp/nrl-saved-regression-replay-new
```


## Real trained small decoder (900-NRL-0012)

The separate [decoder profile](specs/decoder-v0.md) runs the pinned real TinyStories-260K model and its custom byte-BPE tokenizer offline in Rust. Source float32 weights/tables widen to scalar f64; GQA, RMSNorm, tied head, RoPE, causal/cache state and exact package identity are explicit. This is not Qwen or arbitrary Llama support.

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- import-decoder nrl/reference/tinyllamas-260k/stories260K.bin nrl/reference/tinyllamas-260k/tok512.bin nrl/reference/tinyllamas-260k/decoder-source.json /tmp/nrl-decoder-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-decoder /tmp/nrl-decoder-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- probe-decoder /tmp/nrl-decoder-new 'Once upon a time' /tmp/nrl-decoder-output-new.json
```

All output paths must be new. Primary assets and licence notices are in `reference/tinyllamas-260k/`; the optional `tools/generate_decoder_fixtures.py --output tests/fixtures/decoder-v0.json --check` generator uses independent PyTorch and original C calculations. Normal checks need no network, C or Python. Smaller replacement counts do not imply smaller packages: this first profile retains the original dense blob. T07–T10 supply calibration/training/evaluation/plans; native and other families remain separate.

T06 passed **57 host tests** and both portable rlibs; [dated baseline](reports/decoder-baseline-2026-10-26.json), [raw log](reports/decoder-check-2026-10-26.txt), [saved probe](reports/decoder-probes-2026-10-26/0.json). All actual-weight f64 logits/FFN traces and original C tokenizer/f32 checks passed; the oracle regenerates byte-identically. T07–T10 are next.


## Calibration and exact local continuation (900-NRL-0013)

[Language calibration/local-training contract](specs/calibration-local-v0.md). On an imported decoder, the independent operations are:

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- calibrate-decoder /tmp/nrl-decoder-new nrl/tests/fixtures/local-v0/corpus.json 0 64 /tmp/nrl-calibration-new.json
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- train-local /tmp/nrl-decoder-new /tmp/nrl-calibration-new.json nrl/tests/fixtures/local-v0/config.json /tmp/nrl-local-partial-new.json 2
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- resume-local /tmp/nrl-local-partial-new.json /tmp/nrl-local-complete-new.json 8
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-local /tmp/nrl-decoder-new /tmp/nrl-local-complete-new.json /tmp/nrl-local-decoder-new
```

Only train initializes/trains; validation selects; test activations are not collected. Source/corpus/order and initial/current/best parameters are saved, with stateless SGD and explicit no-RNG state. Resume continues the exact saved full-batch run rather than restarting it. Four compressed controls share the train-only source-channel selection; SVD explicitly follows that reduction. Export retains licence/provenance and existing dense storage. Outputs require new paths. Optional independent regeneration: `tools/generate_local_fixture.py --output tests/fixtures/local-v0/expected.json --check` in the pinned reference environment. T08–T10 continue with whole-model metrics, comparison, plans and student-input adaptation.

T07 passed **62 host tests** and both portable rlibs; [dated evidence](reports/local-training-2026-10-26.json), [raw log](reports/local-check-2026-10-26.txt), [saved calibration/checkpoints](reports/local-example-2026-10-26/report.json). Exact continuation and real-model PyTorch local checks passed. T08–T10 use the already frozen study profile.

T08 is complete on the NRL branch (900-NRL-0014): 65 host tests, both portable rlibs, two independent task metrics and compatible repeated timing reports. [Evidence](reports/evaluation-2026-10-26.json). Original dense storage and whole-process RSS are reported honestly; the current linear student loses language validation quality. T09/T10 continue using the unchanged pre-frozen study.

T09 is complete on the NRL branch (900-NRL-0015): 68 tests and both portable rlibs; manual mixed composition and six fixed candidates were checked whole. [Measured search](reports/plans-2026-10-26.json) selected `dense` under the validation quality/time/package rules; negative cases are retained. Test follows selection. T10 student-input recovery continues independently.

## Mixed plans and recovery commands

With a new imported teacher package at `/tmp/nrl-decoder-new`, run from the repository root (all output paths must be new):

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- apply-plan /tmp/nrl-decoder-new nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/study-v0/profile.json mixed-linear-pruned /tmp/nrl-manual-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- search-plans /tmp/nrl-decoder-new nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/study-v0/profile.json /tmp/nrl-search-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- recover-plan /tmp/nrl-decoder-new /tmp/nrl-manual-new/model nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/study-v0/profile.json /tmp/nrl-recovery-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-recovery /tmp/nrl-decoder-new /tmp/nrl-recovery-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- replay-recovery /tmp/nrl-decoder-new /tmp/nrl-recovery-new /tmp/nrl-recovery-replay-new
```

[Measurement contract](specs/evaluation-v0.md), [mixed-plan contract](specs/mixed-plans-v0.md), [student-recovery contract](specs/student-recovery-v0.md). Saved inputs and standalone packages are under `reports/plan-example-2026-10-26` and `reports/recovery-example-2026-10-26`; `complete.json` is written last. A fresh run preserves the previous evidence. Repeated timings can change the deployment recommendation; deterministic recovery parameters/validation histories must replay on the declared execution profile.

## Portable sessions and embedding family

[Runtime boundary](specs/native-boundary-v0.md), [prepared decoder experiment](specs/prepared-decoder-v0.md), [embedding/retrieval contract](specs/embedding-retrieval-v0.md).

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-budgeted-model nrl/reports/runtime-example-2026-10-26/model nrl/tests/fixtures/model-v0/input.json nrl/tests/fixtures/runtime-v0/policy.json /tmp/nrl-budgeted-new.json
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- profile-decoder /tmp/nrl-decoder-new nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/optimization-v0/study.json /tmp/nrl-profile-new.json
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- import-embedding nrl/tests/fixtures/embedding-v0/definition.json /tmp/nrl-embedding-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- prune-embedding /tmp/nrl-embedding-new body 2 nrl/tests/fixtures/embedding-v0/replacement-source.json /tmp/nrl-embedding-pruned-new
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- encode-embedding /tmp/nrl-embedding-new 'moon sky' /tmp/nrl-vector-new.json
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-retrieval /tmp/nrl-embedding-new /tmp/nrl-embedding-new nrl/tests/fixtures/embedding-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json /tmp/nrl-embedding-baseline-new.json
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-retrieval /tmp/nrl-embedding-new /tmp/nrl-embedding-pruned-new nrl/tests/fixtures/embedding-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json /tmp/nrl-embedding-candidate-new.json
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- compare-reports /tmp/nrl-embedding-baseline-new.json /tmp/nrl-embedding-candidate-new.json /tmp/nrl-embedding-comparison-new.json
```

Output paths must be new. The saved standalone baseline/pruned encoders, original index/query data, policy and reports are in `reports/embedding-example-2026-10-26/`. Optional PyTorch reference regeneration is `tools/generate_embedding_fixture.py --output-dir tests/fixtures/embedding-v0 --check` in the pinned environment; normal Rust use requires no Python/download. Projection transforms reuse the previous model API; lookup/pooling/L2 backward and native execution are unsupported and never silently delegated to another backend.

## AdamW and refreshed recovery (900-NRL-0020)

A shared bounded named-tensor AdamW implements bias correction, decoupled decay and parameter-bound moments. Local multi-layer recovery refreshes teacher targets on student train activations; validation NLL selects checkpoints. It does not train attention or backpropagate the full decoder loss. The paired optimizer command writes complete checkpoints, selected models and held-out reports with a dense fallback. [Contract and CLI](specs/adamw-recovery-v0.md).

```bash
nrl/target/release/nrl-lab compare-adaptation BASE INITIAL CORPUS nrl/tests/fixtures/adaptation-v0/config.json NEW_DIRECTORY
```

## Prepared incremental generation (900-NRL-0021)

Checked FFN preparation and scratch can be reused throughout a greedy generation request. A prepared session rolls back appended KV tails on a failed step; the public externally supplied-state step retains its existing semantics. The reference path remains available. [Generation contract and paired measurement](specs/prepared-generation-v0.md).

## End-to-end embedding training (900-NRL-0022)

Lookup, mean pooling, shared projection and L2 normalization now have an analytic backward path. A bounded graded contrastive objective trains all table/projection weights with the shared AdamW; complete checkpoints resume exactly and validation loss selects the standalone export. Retrieval of trained tables uses the distinct `embedding-trained-retrieval/0` task; the original projection-only task still freezes the table. All index documents participate in training, while query splits remain separate. [Contract, independent oracle and CLI](specs/embedding-training-v0.md).

## Complete supported-model QSL binding (900-NRL-0023)

QSL overlays can now bind to the exact supported Tinyllamas decoder, run the complete model, and export a standalone package containing unchanged weights/tokenizer plus all replacements and provenance. The original QSL writer/loader and independent PyTorch full logits/tokens check four transformed layers. This narrow base-fingerprint policy is explicit; Hugging Face/Qwen architectures and the external HF standalone format remain unsupported. [Binding, overlay, export and commands](specs/qsl-complete-model-v0.md).
