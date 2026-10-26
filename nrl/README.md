# Neural Representation Lab: initial Rust numerical path

NRL is track `NRL`, issue [900](../issues/900-neural-representation-lab.md). This workspace implements a **small host CPU f64 block profile**: dense and pruned SiLU blocks, shared deterministic channel selection, linear and cubic Hermite spline interpolation, direct SVD factor execution, their gated MLPs with optional gate/up/down output biases and analytic gradients. It is not yet a full model runtime or native model service.

## Run from the repository root

```sh
cargo test --manifest-path nrl/Cargo.toml --locked
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target x86_64-unknown-none
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target aarch64-unknown-none-softfloat
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- demo nrl/tests/fixtures/f64-v5.json /tmp/nrl-demo-new
```

The output directory must not exist. The demo checks reference outputs and gradients, takes one SGD step in each fixture and checks a block-file round trip. It writes twelve block files and a JSON report. Errors use a nonzero exit status; existing files are never overwritten. A failed I/O operation can leave incomplete new outputs.

Use `--offline` as well when the locked dependencies are already cached. `nrl/tools/check.sh` runs the Rust checks and a fresh demonstration. Python is not required to run the Rust tests or demonstration.

The project's pinned nightly builds the code. If that toolchain lacks rustfmt, `cargo +stable fmt --manifest-path nrl/Cargo.toml --all --check` checks formatting with an installed stable formatter; it does not select the compiler used by the tests.

## Structure

| Component | Responsibility |
|---|---|
| `crates/nrl-core` | no_std shapes, limits, errors |
| `crates/nrl-repr` | no_std, allocation-free math over explicit borrowed buffers; libm exponential |
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
