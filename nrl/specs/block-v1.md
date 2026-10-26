# NRL block contract v1

**Edition:** 1 (2026-10-26) · **Tasks:** 900-NRL-0002, [900-NRL-0005](../../issues-done/900-NRL-0005-pruning-and-shared-channel-selection.done) · **Scope:** a small CPU float64 numerical path, not a model service or full-model format.

## Block semantics

A block has positive dimensions `input = n`, `channels = w`, `output = m`. Its three row-major, contiguous matrices are:

| Tensor | Shape | Index |
|---|---|---|
| gate G | w × n | G[c*n+i] |
| up U | w × n | U[c*n+i] |
| down D | m × w | D[o*w+c] |

There are no biases in v1. On one input row x:

```text
z[c] = sum_i G[c,i] * x[i]
u[c] = sum_i U[c,i] * x[i]
a[c] = activation(c, z[c])
y[o] = sum_c D[o,c] * a[c] * u[c]
```

`dense` and `pruned` use SiLU: `activation(z) = z * sigmoid(z)`. The derivative is `sigmoid(z) * (1 + z * (1 - sigmoid(z)))`. A sign-stable sigmoid avoids exponentiating a large positive number.

`spline-linear` replaces SiLU with the channel's interpolant below. It still executes the projections; conversion does not remove all matrix products. The first fixture uses the same width for both blocks and makes no compression claim. Pruned and spline controls share the retained-channel mapping below.

`output` may differ from `input`. No shared block contract contains a Qwen layer number, tokenizer, attention layout or family-specific name.

## Uniform linear splines

Each channel has fixed finite `low < high`, a common integer knot count `k >= 2`, and `k` trainable finite values. Grids do not train in v1. Let:

```text
u = (x-low)/(high-low) * (k-1)
j = clamp(floor(u), 0, k-2)
t = u-j
s(x) = values[j] + (values[j+1]-values[j]) * t
s'(x) = (values[j+1]-values[j]) * (k-1)/(high-low)
```

The index clamps; t does not. Both tails extend the nearest segment linearly. At an internal exact knot, the right segment's derivative is used. Endpoints use their adjacent interval, the same slope as the adjacent tail. A central finite difference crossing a kink is not the reference for that one-sided convention.

Coefficient derivatives are `1-t` and `t` for the selected adjacent coefficients, including tails. The stored coefficient index is `channel*k + j`. Grid span and scale must be finite and positive; nonfinite slopes or intermediate values are refused.

**Separate future representations.** In 900 T03, `spline-cubic` means the reference's cubic Hermite interpolation with finite-difference slopes. Task 262 additionally describes B-spline edge functions. Those require a distinct representation ID, mathematical contract and tests; Hermite results do not discharge the B-spline requirement. Neither cubic form is implemented by this v1 path.

## Pruning and shared channel selection

The portable scorer accepts a checked bias-free SiLU block, fixed f64 weights and either no calibration or a nonempty batch of matching input rows. Both methods return nonnegative f64 scores:

```text
weight-energy-f64/0: ||G[c,:]||² * ||U[c,:]||² * ||D[:,c]||²
activation-energy-f64/0: mean_train((SiLU(Gx)[c] * Ux[c])²) * ||D[:,c]||²
```

Only the first 4096 supplied rows contribute to activation ranking. The entire input length must be a valid batch within MAX_ELEMENTS; ignored rows do not contribute to scores. Empty calibration is refused instead of silently becoming weight-only ranking. The caller is responsible for supplying training/calibration data rather than validation or test data. Dataset identity/split manifests are later T04/T07 work.

The top `width` scores are selected, with equal scores preferring the lower source index, and the retained mapping is then sorted by original index. Selection uses a bounded in-place heap over the caller's index buffer, O(channels*log(width)) selection plus sorting; full width returns the identity mapping. No randomness is used. Negative/nonfinite scores, arithmetic overflow, malformed buffers and out-of-range widths are errors. Low-level score buffers can be partially written on failure.

The Python reference forcibly casts weights/calibration to f32 for ranking; this profile deliberately uses f64, names that in its method ID, and does not promise identical choices near numerical ties. Fixtures check original reference indices on unambiguous cases and independent PyTorch f64 scores. Stable exact ties are separately checked.

Packing copies exactly the selected rows of G/U and columns of D, with no refit of D. A spline control starts from this same pruned block, preserving its mapping and projections. Its initial coefficients are SiLU samples on an explicitly supplied per-channel grid; choosing grid quantiles and refitting D remain calibration tasks. The current transform accepts only a dense SiLU source. It never silently treats a spline or an unsupported biased block as that source.

`selection` records `original_channels`, an ascending, unique in-range `indices` array of length `w`, and one of the two method IDs above. It is a declared mapping, not cryptographic source identity or a full provenance manifest. The standalone block still stores all retained weights. Training preserves the mapping.

**Transition:** v1 replaces the initial `nrl.block/0` reader because retained-channel semantics are now explicit. Version 0 is refused with its name; regenerate prototype artifacts from their source. No compatibility reader or automatic reinterpretation is supplied before the first release. Historical reports record the old fixture and remain historical.

## Backward and one training step

Given upstream `dy`, set `dh[c] = sum_o D[o,c]*dy[o]`. The gradients are:

```text
dD[o,c] += dy[o] * a[c] * u[c]
dz[c] = dh[c] * u[c] * activation'(c,z[c])
du[c] = dh[c] * a[c]
dG[c,i] += dz[c] * x[i]
dU[c,i] += du[c] * x[i]
dx[i] = sum_c (dz[c]*G[c,i] + du[c]*U[c,i])
```

Spline coefficient gradients add `dh[c]*u[c] * (1-t, t)` at the selected coefficients. Parameter gradient buffers accumulate across rows; the input gradient is replaced on each call. All buffer lengths are checked. The caller initializes accumulation buffers explicitly.

The host training spike uses `mean((prediction-target)^2)` across every output of every row, so its upstream gradient is `2*(prediction-target)/output_element_count`. A single SGD step uses a finite positive learning rate. It builds and validates a new block before returning it; the original block is unchanged on failure. This is not AdamW, resumable training, a trained-model optimizer or a general autodiff system.

## Bounds, ownership and errors

- The portable crates are `no_std`, contain no unsafe code and allocate no memory. They receive borrowed weights, caller-owned output/gradient buffers and scratch of `3*w` f64 values.
- A shape is refused if multiplication overflows or any checked matrix/scratch count exceeds 1,048,576 elements. These are prototype computational limits, not an OS resource capability or deadline guarantee.
- Host batch input and output counts each have the same element bound. The artifact's stronger bound below also applies.
- Weights are immutable during a call. A future cross-domain adapter must establish stable checked data before constructing these borrowed views (MC-11.2); these Rust types alone do not establish that OS contract.
- Errors distinguish shape, limit, nonfinite values and invalid grid. Arithmetic overflow to infinity is a refusal, not clipping or a success containing NaN.
- After a low-level computation returns an error, output, gradient and scratch buffers may be partially written and must be discarded or reinitialized. It changes no weights. The host owns any transient allocations; this is not a tested native budget implementation.
- No file, network, thread, OS clock or device access belongs to the numerical layer.

## Data-only artifact `nrl.block/1`

The host artifact is a UTF-8 JSON object:

| Field | Meaning |
|---|---|
| schema | Exactly `nrl.block/1`; any other version is refused with its name |
| representation | Exactly `dense`, `pruned` or `spline-linear` |
| shape | `[n,w,m]`, positive dimensions checked before indexing |
| gate, up, down | Flat f64 arrays with the exact matrix lengths |
| spline | For dense/pruned: null or absent. For spline-linear: the object below |
| selection | Required for pruned, optional for spline, absent/null for dense; original_channels, ascending unique indices, method |
| spline.knots | Integer k >= 2 |
| spline.low, high | One fixed finite bound per channel |
| spline.values | Channel-major w × k coefficients |

Unknown or duplicate fields are refused. Nonfinite JSON numbers, inconsistent arrays, invalid grids and unknown representations are refused. The file is at most 1,048,576 bytes, enforced by a bounded read rather than trusting its metadata. The total number of matrix, grid and coefficient values is at most 16,384. JSON parsing has serde_json's default recursion limit as well as the byte bound.

Saving uses a new file and refuses an existing path. Validation and serialization happen before file creation. An I/O error can leave an incomplete new file; the caller treats the operation as failed, and a later loader must validate it again. A collection of output files is not an atomic transaction.

This format stores every parameter needed for this one block. It is not a QSL overlay importer, a standalone language model or a stable Rust ABI. ModelDescriptor, provenance manifests for full models, calibration records and cross-domain requests remain later contracts. No compatibility reader is added for an earlier in-project schema before the first release.

## Numerical acceptance

- Reference f64 comparisons: absolute error <= `1e-12 + 1e-10*abs(reference)`; every compared number must be finite.
- Independent central finite differences away from knots: epsilon `1e-6`, absolute/relative tolerances `2e-7 + 2e-5*abs(analytical_gradient)`.
- Exact knots and tails have separate explicit checks.
- Save/load preserves every f64 parameter and the forward outputs exactly on the tested host through round-trip float parsing.
- A fixture step must match the reference's new loss and lower this fixed synthetic loss. That establishes no gain on a real model or held-out task.
- These checks are not proofs; results apply to the recorded host/backend/dtype. There is no bitwise cross-backend or hardware guarantee (MC-12.1, 12.2).

## Service boundary

APP owns 262's `nnd`, its service IDL, sessions, authority and budgets. NRL supplies libraries, representation contracts and artifacts. The initial host runner uses ordinary files and has no native service stub presented as working integration. A native adapter and its early smoke test are separate work. Accelerators and platform changes remain KRN/DRV tasks under 178.
