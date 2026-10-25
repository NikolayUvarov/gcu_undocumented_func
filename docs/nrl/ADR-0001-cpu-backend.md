# ADR 0001: a narrow scalar f64 CPU path for the first NRL block

**Status:** accepted for the initial block profile, 2026-10-25 · **Task:** [900-NRL-0003](../../issues-done/900-NRL-0003-cpu-backend-spike.done).

## Context

The first experiment needs a dense baseline, a linear-spline gated block, independently checked gradients and one training step. It must compile without std for MIND Core before a model adapter or nnd exists. A general GPU or training framework is not needed to check those numerical semantics.

The existing `mind::nn` is a no-system-call CPU inference interpreter, with an optimized `gemm`. It is not an autodiff/training implementation. No changes to APP's interpreter or the kernel are made by this spike.

## Decision

- `nrl-core`: checked shapes, element limits, finite-value errors. No dependencies.
- `nrl-repr`: borrowed buffers, scalar f64 matrix loops, dense SiLU, fixed-grid linear splines and analytic gradients. Its only external runtime dependency is `libm = 0.2.16`, used for the exponential.
- `nrl-lab`: std host adapter, bounded JSON block files, a small batch evaluator, one SGD step and a demonstration command. It depends on locked serde/serde_json and the portable crates.
- The portable crates are no_std, allocate no memory, and forbid unsafe code in their own source. Limits and buffers are explicit. This is a first numerical implementation, not the final high-performance backend.

The required commands and the exact dependency graph are recorded in `nrl/reports/cpu-f64-2026-10-25.json` and `nrl/Cargo.lock`. Cargo's locked checksums bind registry packages to their downloaded content.

## Evidence

On the recorded Linux x86-64 host:

- Dense and linear-spline CPU forward, backward and one SGD step executed through the host runner.
- Each trained loss matched the original Python reference; both fixed synthetic losses decreased.
- The release portable libraries were built for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`. This builds rlibs, not a linked or running MIND Core application.
- The portable dependency graph is `nrl-repr -> nrl-core + libm 0.2.16`; there is no std, BLAS, CUDA or external C build dependency in this graph. NRL's portable source imports no foreign functions. A source search of libm found a local Rust function with a C calling convention in argument reduction; it is a definition, not a foreign-library import.
- These checks do not audit all generated compiler support routines, establish runtime linkage, run under native scheduling or validate an accelerator.

## Alternatives and costs

Burn and Candle remain candidates from issue 900; neither was compiled or benchmarked in this spike, and neither is rejected as incompatible. Selecting a whole framework now would introduce dependency and platform questions before the first two block semantics are checked. Revisit a framework when its operations and training machinery materially reduce the remaining work, with its exact revision, feature/FFI audit and target tests.

Reusing `mind::nn`/`gemm` directly would connect a numerical experiment to APP's production inference code before its training contracts exist. Their kernels remain candidates for a measured later CPU backend through the owning track.

The narrow implementation costs NRL maintenance of its backward passes and matrix operations. Independent finite differences and external reference tensors are therefore required. There is no claim that it beats a mature backend or that f64 is the final inference precision.

## Consequences and remaining work

Only dense and spline-linear bias-free blocks are present. Pruned, cubic Hermite, SVD, other operators/dtypes, model adapters, optimizer state/resume and the native service remain open. Host use of std files and clocks is explicit; it grants no native capability or budget enforcement.

CI wiring belongs to KRN's scripts and is requested separately. Until that request lands, NRL's commands must be run explicitly; the existing full gate does not automatically discover this workspace. Code is submitted through the project's landing queue.
