# Portable importer differential check 0 (2026-10-30)

Task: 900-NRL-0026; roadmap 900 T11. Report schema `nrl.portable-import-check/0`. This is a host tool linking APP's actual `nnpkg` implementation. It exercises the exporter/importer and portable service together; it does **not** run `nnd` in MIND or establish IPC authentication, worker isolation, VFS scopes or native memory cleanup.

## What changed in the service dependency

NRL's pinned handoff 0025 landed on 2026-10-29 (round 44). APP's reply on internal-dev-messages, `ALL/2026-10-29T1612Z-APP-nnd-badge-retirement-suits-apps-design-at-1-0.md`, accepts the format/limits/accounting/lifetime clarifications and explicitly owns its native parser/encoders/views. `nnpkg/`, `nnd/`, `idl/nn.wit` 1.0 and generated bindings are now on main. KRN's boot/grant work landed in round 50, `8e2b16e0`, with ABI/handoff 25 and `SLOT_NN` 41. The boot test named `nnd` checks grants/quota/workers; it does not yet execute the pinned model through a native client.

The remaining native dependency is APP's shell mint/revoke/retire lifecycle, client commands/harness, and `nnd` selecting its two granted file scopes. 262-APP-0112 and APP's remaining part of 262-KRN-0178 are still open. NRL must not keep reporting the whole service as absent, nor close 0017 from a service boot or host test.

## Command

```bash
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-native-model nrl/tests/fixtures/native-conformance-v0/model nrl/tests/fixtures/model-v0/input.json /tmp/new-portable-check.json
```

Arguments are an existing standalone model directory, a JSON array of input f64 values and a **new** report path. Existing reports are never overwritten. The normal check is offline once locked Rust dependencies are installed. The name means checking compatibility with the native implementation, not executing natively; the report always says `native_execution: false`.

Inputs must be finite, nonempty, shape-correct and within the model's row/activation limits. Both input and output must fit `nn.wit`'s 65536-byte payload limit (8192 binary64 values). Input JSON and object reads are capped at 1 MiB; the existing standalone model loader bounds the manifest, object count and total package. These ordinary host file checks make no adversarial-filesystem guarantee. This command checks successful numerical execution; the pinned interruption/numerical-error suite remains a separate profile.

## Paths compared

1. NRL's strict host loader reads the current export and computes its model identity. Every covered file is read within its existing cap.
2. APP's `package::load` reads those exact files, checks their hashes/lengths, typed writer encoding and all portable graph/profile bounds.
3. The separate parser-answer path uses APP's `parse_manifest/read_manifest` and `parse_object/read_object`, then assembles the result. Its worker description must equal the direct loader's description.
4. APP's `Package::from_description` reconstructs the job-worker description. Its serialization and model ID must round trip exactly. These are APP's linked implementation APIs, not a newly standardized external protocol.
5. NRL's and APP's prepared programs must agree on graph output, row/scratch/output layout, work per row, charged rows/work/requests and checkpoint count. Both execute in caller-owned buffers through `nrl-runtime`; workspace must be cleared after success. Outputs must match NRL's `Model::forward` within the existing finite/count-checked tolerance `1e-12 + 1e-10*abs(reference)`.
6. APP's portable `Service` runs `insert/open/submit/start/execute/finish/take`, with an in-process fake worker/clock. Before completion, output is pending; it is taken exactly once. The usage must match, then `close/unload` must release portable service records/accounting to zero.

The comparison oracle is NRL's host model execution. Many kernels are shared, so this is evidence about parsing, translation and integration, **not a new independent mathematical oracle**. The original fixed model still has its separate independent PyTorch oracle and existing 0025 checks. Matching portable accounting is not native heap/page accounting. The trusted host owner 41 is a supplied number, not a kernel-authenticated client badge. The fake clock increments at checkpoints and establishes no timing/preemption bound.

## Differential corpus

The NRL integration tests construct **70 fresh valid exports** through the real writer:

- 62 transformed blocks: dimensions `(1,1,1)`, `(2,3,2)`, `(3,4,2)`, `(5,6,4)`; optional projection biases on/off; dense, pruned, linear spline (2/5 knots), cubic spline (3/6 knots), SVD ranks 1/2 where applicable.
- Three valid output subgraphs from the composed model. Each removes unused nodes/objects and recomputes ties, as model-v0 requires; input-only outputs are explicitly refused by the exporter.
- Unicode/control/escaped provenance and classification-logit task labels.
- A materialized standalone overlay with nonempty change history.
- Two affine objects at exactly 16384 stored values, with and without bias.
- A chain at the 64-object/64-node boundary.

Refusals cover empty/bad-shape/nonfinite/over-batch inputs, oversized IPC output, existing report publication and host-valid noncanonical object bytes. The general host loader's broader accepted JSON remains unchanged; the portable importer explicitly requires `nrl.serde-json-pretty-object/0`. No requirement or test is weakened to make either implementation agree.

Standard `nrl/tools/check.sh` also checks freshly packed, materialized-overlay and actually trained/selected regression models through the new CLI. APP's existing `nnpkg` suite remains separately owned and is run unchanged; it covers the fixed independent fixture, encoding, resource/cancellation/poisoning/retirement and parser refusal cases.

## Native work still open

APP finishes the scoped-file selection and shell's retained capability, revoke/retire/retire-all, `nn` client and `nncheck` suite. Then run real nnd loads/inference through IPC on named targets: independent outputs/tolerance, cumulative budgets, controlled partial cancellation/deadlines, two-client isolation, finite-input internal failure/new session, badge lifetime/reuse and native memory after unload. Record the exact configuration/commit/TCB/raw logs in the profile. This host check and bare-metal library builds close none of those native criteria in 900-NRL-0017.
