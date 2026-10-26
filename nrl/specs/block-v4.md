# NRL block contract v4

**Edition:** 4 (2026-10-26) · **Tasks:** 900-NRL-0002, [900-NRL-0005](../../issues-done/900-NRL-0005-pruning-and-shared-channel-selection.done), [900-NRL-0006](../../issues-done/900-NRL-0006-cubic-hermite-block.done), [900-NRL-0007](../../issues-done/900-NRL-0007-factorized-svd-blocks.done), [900-NRL-0008](../../issues/900-NRL-0008-affine-bias-and-applicability.md) · **Scope:** a small CPU float64 numerical path, not a model service or full-model format.

## Block semantics

A block has positive dimensions `input = n`, `channels = w`, `output = m`. Its three row-major, contiguous matrices are:

| Tensor | Shape | Index |
|---|---|---|
| gate G | w × n | G[c*n+i] |
| up U | w × n | U[c*n+i] |
| down D | m × w | D[o*w+c] |

Each projection has an optional independent output bias: bg/bu have w entries and bd has m. An absent projection bias is zero. On one input row x:

```text
z[c] = sum_i G[c,i] * x[i] + bg[c]
u[c] = sum_i U[c,i] * x[i] + bu[c]
a[c] = activation(c, z[c])
y[o] = sum_c D[o,c] * a[c] * u[c] + bd[o]
```

`dense` and `pruned` use SiLU: `activation(z) = z * sigmoid(z)`. The derivative is `sigmoid(z) * (1 + z * (1 - sigmoid(z)))`. A sign-stable sigmoid avoids exponentiating a large positive number.

`spline-linear` replaces SiLU with the channel's interpolant below. It still executes the projections; conversion does not remove all matrix products. The first fixture uses the same width for both blocks and makes no compression claim. Pruned and spline controls share the retained-channel mapping below.

`output` may differ from `input`. No shared block contract contains a Qwen layer number, tokenizer, attention layout or family-specific name.

## Uniform linear splines

Each channel has fixed finite `low < high`, a common integer knot count `k >= 2`, and `k` trainable finite values. Grids do not train in v4. Let:

```text
u = (x-low)/(high-low) * (k-1)
j = clamp(floor(u), 0, k-2)
t = u-j
s(x) = values[j] + (values[j+1]-values[j]) * t
s'(x) = (values[j+1]-values[j]) * (k-1)/(high-low)
```

The index clamps; t does not. Both tails extend the nearest segment linearly. At an internal exact knot, the right segment's derivative is used. Endpoints use their adjacent interval, the same slope as the adjacent tail. A central finite difference crossing a kink is not the reference for that one-sided convention.

Coefficient derivatives are `1-t` and `t` for the selected adjacent coefficients, including tails. The stored coefficient index is `channel*k + j`. Grid span and scale must be finite and positive; nonfinite slopes or intermediate values are refused.

## Cubic Hermite splines

`spline-cubic` uses the same grid, interval and trainable values as the linear variant. For an interior point in interval j, define y0=v[j], y1=v[j+1], m0=(y1-v[j-1])/2 and m1=(v[j+2]-y0)/2. At the first interval use m0=y1-y0; at the last use m1=y1-y0. Then:

```text
H00 = 2*t³ - 3*t² + 1
H10 = t³ - 2*t² + t
H01 = -2*t³ + 3*t²
H11 = t³ - t²
s = H00*y0 + H10*m0 + H01*y1 + H11*m1
```

The slopes are in interval coordinates; the input derivative includes `(k-1)/(high-low)`. Coefficient derivatives include the dependence of both slopes on neighbouring trainable values, up to four distinct coefficients. The sample returns their indices and weights; repeated indices at endpoints accumulate. With two knots it reduces to a linear segment.

For u < 0 or u > k-1, use the nearest **linear** segment and its linear coefficient/input derivatives. Do not evaluate an unused cubic polynomial in the tails. Values and first derivatives agree across all knots, including the transition to the tails. No monotonicity or absence of overshoot is promised.

This is the reference's Hermite interpolation, not a B-spline. Task 262's B-spline edge functions need their own representation, contract and tests; these results do not discharge that requirement.

## Pruning and shared channel selection

The portable scorer accepts a checked SiLU gated block with optional projection biases, fixed f64 weights and either no calibration or a nonempty batch of matching input rows. Both methods return nonnegative f64 scores:

```text
weight-energy-f64/1: (||G[c,:]||² + bg[c]²) * (||U[c,:]||² + bu[c]²) * ||D[:,c]||²
activation-energy-f64/1: mean_train((SiLU(Gx+bg)[c] * (Ux+bu)[c])²) * ||D[:,c]||²
```

Only the first 4096 supplied rows contribute to activation ranking. The entire input length must be a valid batch within MAX_ELEMENTS; ignored rows do not contribute to scores. Empty calibration is refused instead of silently becoming weight-only ranking. The caller is responsible for supplying training/calibration data rather than validation or test data. Dataset identity/split manifests are later T04/T07 work.

The top `width` scores are selected, with equal scores preferring the lower source index, and the retained mapping is then sorted by original index. Selection uses a bounded in-place heap over the caller's index buffer, O(channels*log(width)) selection plus sorting; full width returns the identity mapping. No randomness is used. Negative/nonfinite scores, arithmetic overflow, malformed buffers and out-of-range widths are errors. Low-level score buffers can be partially written on failure.

The original Python reference only converts bias-free blocks and forcibly casts weights/calibration to f32 for ranking; this profile deliberately uses f64, names that in its method ID, and does not promise identical choices near numerical ties. Fixtures check original reference indices on unambiguous cases and independent PyTorch f64 scores. Stable exact ties are separately checked. The /1 scoring IDs explicitly extend the heuristic to affine projections, using augmented gate/up row energies or actual biased activations. The shared down bias is constant across channel choices and is excluded from ranking; it is preserved during packing. The affine score is a declared heuristic, not a guarantee of optimal channel selection.

Packing copies exactly the selected rows of G/U and columns of D, the selected gate/up biases and the entire down bias, with no refit of D. A spline control starts from this same pruned block, preserving its mapping and projections. Its initial coefficients are SiLU samples on an explicitly supplied per-channel grid; choosing grid quantiles and refitting D remain calibration tasks. The current transform accepts only a dense SiLU source. It never silently treats a spline or another source activation as that source.

`selection` records `original_channels`, an ascending, unique in-range `indices` array of length `w`, and one of the two method IDs above. It is a declared mapping, not cryptographic source identity or a full provenance manifest. The standalone block still stores all retained weights. Training preserves the mapping.

**Transition:** schema 4 adds affine biases and the explicit semantic descriptor, replacing schemas 0, 1, 2 and 3. A bounded schema-header read refuses older versions by name before parsing fields that only the new version requires. Older versions are refused with their names; regenerate prototype artifacts. No compatibility reader or automatic reinterpretation is supplied before the first release. Historical reports retain their original source commits and fixture hashes.

## Low-rank SiLU blocks

`svd` retains the original channel width and replaces each G/U/D with L*R. L is row-major `[outputs, rank]`, R is row-major `[rank, inputs]`; apply R first, then L, then add the projection bias. No hidden rank-space bias is inserted. The host profile requires a common positive rank no greater than min(n,w,m). Numerical projections may have independent valid ranks. No transpose is implicit.

All weights remain factored during forward/backward. Parameter gradients are dL += dy*(Rx)^T, dR += (L^T*dy)*x^T, db += dy, and dx += R^T*(L^T*dy). The low-level projection accumulates dx; the gated block resets dx and accumulates its gate/up paths, matching the compact block's public behaviour. Reconstruction is an explicit diagnostic into a separate caller-provided buffer, never a hidden execution step. Gradients, input/output and scratch sizes and finite values are checked.

The host initializer `svd::decompose` uses one-sided Jacobi rotations on a scaled f64 matrix, transposing wide matrices so the working matrix is tall. Singular values are sorted descending; factors split each retained singular value's square root between L/R. Signs and bases of degenerate singular subspaces need not match another SVD implementation; reconstruction and truncation residuals are compared instead. This is plain truncated SVD, not SVD-LLM or activation-aware factorization.

Initialization is deliberately bounded: each dimension is 1..=64, at most 128 sweeps, relative column-correlation stopping threshold 1e-14. A caller can lower the sweep limit within 1..=128. Nonconvergence, invalid shapes/rank, nonfinite input and numerical overflow are errors. This is a small CPU initializer, not a solver for a large model. The original Python initializer casts to f32; the Rust initializer and its independent singular-value oracle are f64. Stored block forward/backward fixtures use the unchanged reference in f64, separately from initialization.

Both compact and factored forms preserve independent optional gate/up/down output biases. Full-rank SVD preserves those biases exactly; truncation changes the matrix only. Bias gradients accumulate with the other trainable parameters. A missing bias parameter has an empty gradient, not a newly trainable zero. The original QSL conversion still refuses biases; the new affine support is an explicitly checked Rust extension, with its own PyTorch oracle.

A rank can cost more parameters than a small dense matrix: a factored matrix stores rank*(inputs+outputs) values. The small fixture's rank is for numerical checking, not a compression result. Full-model size, quality and speed need separate measurements.

## Backward and one training step

Given upstream `dy`, set `dh[c] = sum_o D[o,c]*dy[o]`. The gradients are:

```text
dD[o,c] += dy[o] * a[c] * u[c]
dz[c] = dh[c] * u[c] * activation'(c,z[c])
du[c] = dh[c] * a[c]
dG[c,i] += dz[c] * x[i]
dU[c,i] += du[c] * x[i]
dbg[c] += dz[c]
dbu[c] += du[c]
dbd[o] += dy[o]
dx[i] = sum_c (dz[c]*G[c,i] + du[c]*U[c,i])
```

Spline coefficient gradients add `dh[c]*u[c] * (1-t, t)` at the selected coefficients. Parameter gradient buffers accumulate across rows; the input gradient is replaced on each call. All buffer lengths are checked. The caller initializes accumulation buffers explicitly.

The host training spike uses `mean((prediction-target)^2)` across every output of every row, so its upstream gradient is `2*(prediction-target)/output_element_count`. A single SGD step uses a finite positive learning rate. It builds and validates a new block before returning it; the original block is unchanged on failure. This is not AdamW, resumable training, a trained-model optimizer or a general autodiff system.

## Bounds, ownership and errors

- The portable crates are `no_std`, contain no unsafe code and allocate no memory. They receive borrowed weights, caller-owned output/gradient buffers and scratch of `3*w` f64 values for compact blocks or `7*w + max_projection_rank` for SVD blocks. A single factored projection uses `rank` scratch values.
- A shape is refused if multiplication overflows or any checked matrix/scratch count exceeds 1,048,576 elements. These are prototype computational limits, not an OS resource capability or deadline guarantee.
- Host batch input and output counts each have the same element bound. The artifact's stronger bound below also applies.
- Weights are immutable during a call. A future cross-domain adapter must establish stable checked data before constructing these borrowed views (MC-11.2); these Rust types alone do not establish that OS contract.
- Errors distinguish shape, limit, nonfinite values and invalid grid. Arithmetic overflow to infinity is a refusal, not clipping or a success containing NaN.
- After a low-level computation returns an error, output, gradient and scratch buffers may be partially written and must be discarded or reinitialized. It changes no weights. The host owns any transient allocations; this is not a tested native budget implementation.
- No file, network, thread, OS clock or device access belongs to the numerical layer.

## Data-only artifact `nrl.block/4`

The host artifact is a UTF-8 JSON object:

| Field | Meaning |
|---|---|
| schema | Exactly `nrl.block/4`; any other version is refused with its name |
| descriptor | Required semantic descriptor; fields and applicability below |
| bias | Null/absent for no biases; otherwise gate/up/down f64 arrays, each empty or exact projection output length |
| representation | Exactly `dense`, `pruned`, `spline-linear`, `spline-cubic` or `svd` |
| shape | `[n,w,m]`, positive dimensions checked before indexing |
| gate, up, down | Compact: flat f64 arrays with the exact matrix lengths. SVD: empty arrays |
| spline | For dense/pruned/SVD: null or absent. For either spline: the object below |
| selection | Required for pruned, optional for spline, absent/null for dense and SVD; original_channels, ascending unique indices, method |
| factors | SVD only: required rank and gate_left/right, up_left/right, down_left/right arrays; null/absent for compact blocks |
| factors.gate_left, up_left | w × rank |
| factors.gate_right, up_right | rank × n |
| factors.down_left | m × rank |
| factors.down_right | rank × w |
| spline.knots | Integer k >= 2 |
| spline.low, high | One fixed finite bound per channel |
| spline.values | Channel-major w × k coefficients |

Unknown or duplicate fields are refused. Nonfinite JSON numbers, inconsistent arrays, invalid grids and unknown representations are refused. The file is at most 1,048,576 bytes, enforced by a bounded read rather than trusting its metadata. The total number of matrix, factor, bias, grid and coefficient values is at most 16,384. JSON parsing has serde_json's default recursion limit as well as the byte bound.

Saving uses a new file and refuses an existing path. Validation and serialization happen before file creation. An I/O error can leave an incomplete new file; the caller treats the operation as failed, and a later loader must validate it again. A collection of output files is not an atomic transaction.

This format stores every parameter needed for this one block. It is not a QSL overlay importer, a standalone language model or a stable Rust ABI. ModelDescriptor, provenance manifests for full models, calibration records and cross-domain requests remain later contracts. No compatibility reader is added for an earlier in-project schema before the first release.

## Numerical acceptance

- Reference f64 comparisons: absolute error <= `1e-12 + 1e-10*abs(reference)`; every compared number must be finite.
- Independent central finite differences away from knots: epsilon `1e-6`, absolute/relative tolerances `2e-7 + 2e-5*abs(analytical_gradient)`.
- Exact knots and tails have separate explicit checks.
- Save/load preserves every f64 parameter and the forward outputs exactly on the tested host through round-trip float parsing.
- A fixture step must match the reference's new loss and lower this fixed synthetic loss. That establishes no gain on a real model or held-out task.
- These checks are not proofs; results apply to the recorded host/backend/dtype. There is no bitwise cross-backend or hardware guarantee (MC-12.1, 12.2).

## Descriptor and capability matrix

The required descriptor declares `kind`, `source_activation`, `dtype`, `layout`, `state` and `parameter_sharing`. It describes the source block's semantics; the representation explicitly chooses whether that source activation is retained or replaced by a declared spline. No family-specific name is embedded here.

| Dimension | This profile accepts | Other requests |
|---|---|---|
| Block kind | gated-feed-forward/0 | Refused by name; not inferred from shapes |
| Source activation | silu | Refused; no silent activation replacement |
| Dtype / layout | f64 / row-major | Refused; no hidden cast or transpose |
| State | stateless | Refused; no implied KV or recurrent state support |
| Parameter sharing | independent | Refused; tied parameters need a separate aliasing contract |
| Device requested through block_on | cpu-scalar | Refused; no hidden device fallback |
| Projection biases | absent or independent gate/up/down output vectors | Wrong lengths or nonfinite values refused |
| Representations | dense, pruned, spline-linear, spline-cubic, svd | Refused |
| Backward | All five with explicit gradient buffers | No general autodiff or other operator coverage claimed |

`block()` explicitly constructs this CPU profile; `block_on(device)` checks a caller's requested device first. The descriptor is checked data, not evidence that a future importer correctly identified a graph or that a native device is authorized. Model graph validation, operator registries and native budgets remain their own work.

The host demo emits `nrl.demo-report/1`: each case includes the count of trainable bias values alongside its reference error, before/after loss and row count. Elapsed time is fixture checking, not a model benchmark. Five unmodified QSL cases and seven explicitly marked affine/rectangular PyTorch extension cases have separate provenance tags in the fixture.

## Service boundary

APP owns 262's `nnd`, its service IDL, sessions, authority and budgets. NRL supplies libraries, representation contracts and artifacts. The initial host runner uses ordinary files and has no native service stub presented as working integration. A native adapter and its early smoke test are separate work. Accelerators and platform changes remain KRN/DRV tasks under 178.
