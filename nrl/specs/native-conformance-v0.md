# Native conformance handoff 0 (2026-10-29)

Issue: 900-NRL-0025; native integration remains **900-NRL-0017 / 262-APP-0112 / 262-KRN-0178**. This is a pinned host fixture, an independent numerical oracle and proposed native acceptance cases. It is not native execution, an implemented service interface, process isolation evidence or an OS profile guarantee.

## Reproduce offline

From the repository root, with the locked Rust toolchain/dependencies already installed:

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-native-fixture nrl/tests/fixtures/native-conformance-v0
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- make-native-fixture /tmp/new-nrl-native-fixture
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-native-fixture /tmp/new-nrl-native-fixture
diff -r nrl/tests/fixtures/native-conformance-v0 /tmp/new-nrl-native-fixture
```

The export directory must not exist. Normal generation/checks do not use Python, a network or runtime inference to produce the expected outputs. The checker reconstructs pinned expected bytes, checks the exact inventory and refuses symlinks/extra files; a modified caller-supplied bundle index cannot authorize new payloads. These ordinary host file checks are not a native capability or an adversarial-filesystem guarantee. A failed write may leave an incomplete new directory; `bundle.json` is written last.

## Pinned model and numerical acceptance

[Bundle](../tests/fixtures/native-conformance-v0/bundle.json) schema: `nrl.native-conformance/0`. Model ID:

```text
1b23aaf4baa75fa146ee8618ea1f7e0e64699c1ea2ee7cf27eeb8bc83f0c1bee
```

Copy only `model/` to the native read-only model store; the other bundle files are client/harness material. The unchanged `nrl.model/0` standalone package has 7 objects, 15 nodes and 287 stored numerical values. It covers affine, SiLU, ReLU, add, a shared affine object and all five supported gated paths. It takes 3 scalars per input row and produces 2 per output row. Its execution profile remains `nrl.execution/0`, scalar CPU, **f64 only**.

The original synthetic source and oracle are pinned to commit `11547f61a75d6d1efb5c001439cfccb7cd3d3189`. The bundle includes their SHA-256 identities, full MIT licence and independent PyTorch 2.10.0+cpu reference (`reference.json`, field `original_output`). The generator is `nrl/tools/generate_model_fixtures.py` at that commit; its reference archive/environment checks remain applicable. No pretrained or proprietary weights are introduced. The new fixture/tool commit is recorded separately in the evidence and handoff message to avoid a self-referential commit identity.

`input.f64le` is 9 little-endian IEEE-754 binary64 values, row major (3 rows). `expected.f64le` contains the 6 independent reference values. Require equal element counts, all finite, and for **each** element:

```text
abs(actual - reference) <= 1e-12 + 1e-10 * abs(reference)
```

This is an acceptance criterion to execute on each named native configuration. Host success does not establish native success, bitwise portability, or an error bound for other models. Signed-zero bits are significant for encoding checks, not for this arithmetic tolerance.

`numeric-input.f64le` is one finite row `[f64::MAX, -f64::MAX, f64::MAX]`; this model overflows internally. `long-input.f64le` repeats the three bounded ordinary rows to reach 256 rows (6144 input bytes; 4096 output bytes). Both fit the proposed 64 KiB IPC payload bound. A native cancellation race is not resolved by this size: the job can finish before the client cancels. The harness needs controlled start/checkpoint synchronization, or must report a completed-before-cancel outcome separately; a successful already-completed job does not pass a partial-cancellation test.

## Object writer encoding agreement

NRL agrees with APP design 0.2 on keeping existing text object files and f64 execution. Name the existing **writer subset** `nrl.serde-json-pretty-object/0`; no model-manifest field, model ID, dtype, mathematics or system/service ABI changes here. The host `nrl.model/0` reader continues to allow other valid object whitespace/order when the raw-byte hash/length and derived model ID agree. A native parser/re-encoder that requires writer-exact bytes has a narrower accepted input profile and must name it explicitly; it must not call all other host-valid JSON corrupt.

The reference writer is typed `ObjectData` serialization with locked `serde_json 1.0.151`, `float_roundtrip`, formatter `zmij 1.0.23`. It emits two-space indentation, LF, no final newline, declared struct field order, enum `kind` before `data`, and string-key maps in sorted order. See all seven exported model objects for the supported representation fields, including null/optional fields. Encoding a generic JSON map can reorder struct fields and does not reproduce the typed writer.

[object-encoding.json](../tests/fixtures/native-conformance-v0/object-encoding.json) is a valid affine object with edge-case weights; [encoding-bits.json](../tests/fixtures/native-conformance-v0/encoding-bits.json) gives the exact f64 bit patterns. The vector's pinned SHA-256 is `31505851b9d9a72ffbfdd1685c2d8e194245fc974c4d4f887ffedd5500ec04ca`. Cases include positive/negative zero, integer-valued f64, smallest positive/negative subnormals, minimum normal, maximum finite, exponent thresholds and adjacent-to-one rounding. “Shortest round-trip decimal” alone is insufficient: the existing writer emits `1.0`, `-0.0`, `1e+20`, `0.00001`, and `1e-6` in the supplied cases. Native conversion must recover the exact original bits, including subnormals and negative zero; reject nonfinite/range errors instead of rounding to zero/infinity silently.

`check_object_encoding` is a bounded duplicate-key/unknown-field/numerical validation plus exact typed re-encoding **host oracle**, not a new native decoder. `nrl-lab`'s checked JSON/model loaders and `runtime_bridge::Prepared` require std/host allocation. Only `nrl-core`, `nrl-repr` and `nrl-runtime` are currently portable no_std/no_alloc crates. APP still needs to implement its isolated native parser, canonical encoder and validated borrowed-view construction, or record a separate NRL request for that work. No existing std reader may be described as already linked into MIND.

A future writer that changes these bytes needs a new encoding-profile version and an explicit refusal/transition for the replaced version, coordinated with APP; no pre-release compatibility reader is promised. Individual parameter caps still apply: **16384 values per artifact/affine object** (including affine bias), 262144 resolved model values, and 1 MiB object JSON. A text-capacity estimate of roughly 40000 values is not an accepted object bound. Model/graph/operator/shape/representation validation remains mandatory after decoding; correct float text alone is insufficient.

## Runtime behavior to preserve in nnd

The program needs **47 row values + 30 scratch values = 77 f64 (616 bytes)** of caller-owned workspace. Work is 631 units per row, 1893 for the three-row request. These are linked-runtime accounting units, not elapsed CPU time or a hard preemption bound. The adapter must implement the documented stable trusted kernel-cost policy; service limits may be stricter and should be recorded separately.

The host checker exercises these pinned scenarios:

| Scenario | Result / state | Charged rows / work / requests |
|---|---|---|
| Three ordinary rows | success / Ready; independent output match | 3 / 1893 / 1 |
| Wrong owner or model | Owner or Model / Ready | 0 / 0 / 0 |
| Nonfinite input | Numeric / Ready (preflight) | 0 / 0 / 0 |
| Three rows with work budget 1892 | Limit / Ready; a smaller request can still run | 0 / 0 / 0 |
| Cumulative row or work budget | 2 rows succeed, next 2 refused, then 1 succeeds; next refused / Ready | 3 / 1893 / 2 |
| Maximum 2 requests | next request refused / Ready | 3 / 1893 / 2 |
| Workspace 76 or batch 257 | Limit / Ready | 0 / 0 / 0 |
| Cancel at checkpoint 20 | Cancelled / Cancelled | 2 / 646 / 1 |
| Deadline tick 20 | Deadline / Cancelled | 2 / 646 / 1 |
| Clock reverses at checkpoint 20 | Clock / Failed | 2 / 646 / 1 |
| Finite overflow row | Numeric / Failed | 1 / 39 / 1 |
| Reuse terminal session | Closed; no additional charges | unchanged |
| New independent session after failure | success / Ready | 3 / 1893 / 1 |
| Deadline tick 0 | Deadline / Cancelled before execution | 0 / 0 / 0 |

Checkpoint 20 occurs after the first row's internal output copy and after charging/executing the first affine node of the second row. All failed output is cleared, including that first row. The fake clock/checkpoints make host interruption reproducible; they do not show native scheduler behavior or IPC authentication. Two OS clients with independently badged capabilities and same numeric session IDs must still be tested natively.

Clarifications for APP's design:

1. The runtime preflights the **whole request's** rows/work against remaining cumulative quotas. With stable trusted kernel costs, work-budget exhaustion halfway through an accepted request is unreachable; test whole-request refusal and cumulative exhaustion. Partial cancel/deadline/numeric failure retains already-started row/request/node charges; these are not refunded.
2. Workspace is zeroed on every return. Output is zeroed on **error**, while successful output remains until nnd copies/publishes it. The service then clears private temporary storage after copying; never erase success before the reply.
3. Nonfinite client input should be rejected as malformed/preflight input. Runtime `Numeric` before computation leaves Ready/zero charges; finite-input internal numerical failure marks Failed. Do not poison every session merely because both map to one numerical error label.
4. Native `deadline-ms = 0` meaning “no deadline” needs an explicit checked adapter conversion. Runtime `deadline_tick = 0` means immediate expiry; use a documented absolute no-deadline sentinel such as `u64::MAX`, check duration/absolute overflow and native monotonic clock semantics.
5. 262-KRN-0178 boot grants/RO models/logd/quota/workers/fresh client badges are required for a genuine service test. 262-KRN-0179 shared-word wait is an optimization for later; the proposed worker-per-job stage need not wait for it.

## Remaining native acceptance

APP should freeze the versioned IDL and implement bounded native import, owner/model binding from kernel-supplied badges, per-client/session quotas, worker publication/cancellation/deadline mapping and lifetime cleanup. KRN supplies the agreed boot grants. Then run the pinned model through actual nnd on a named MIND target: expected ID/output/tolerance, corrupted and unsupported objects, whole-request/cumulative resource limits, controlled partial cancellation/deadline, two-client isolation, internal failure/new session and memory baseline after unload. Record target/commit/configuration/TCB and raw logs in the OS profile. Until then, T11's native checkboxes remain open.
