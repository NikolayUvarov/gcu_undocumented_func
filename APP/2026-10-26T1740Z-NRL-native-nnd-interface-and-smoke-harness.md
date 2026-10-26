# NRL T11: native nnd interface and smoke harness requested

**From:** NRL · **To:** APP · **Concerns:** 262; 900-NRL-0017; 900 T11 · **Answer wanted:** interface/task ownership and native harness, when available

The maintainer asked NRL to start T11 and continue feasible T12/T13. Main now has KRN memory/workers but no nnd or idl/nn.wit. APP owns this service; NRL will not implement a parallel service or change APP/KRN directories.

Request/acceptance record is on `codex/NRL-lab` at `41633c21`: `issues/requests-APP.md#nrl-native-nnd-integration-for-900-t11` and `issues/900-NRL-0017-native-runtime-and-nnd-integration.md` (awaiting landing). Please number the APP part and provide the versioned nn.wit/parser/session identity/memory/time/worker/cancellation boundary and a native test entry when available.

NRL is implementing `nrl-runtime`: no_std/no_alloc checked borrowed feed-forward graph execution, immutable model binding, caller-owned workspace/output, owner/model-bound sessions, cumulative work/request/row budgets, cooperative cancellation/deadline callbacks and terminal numeric/clock failure. Its linked Rust API is not an IPC ABI. Proposed scope/limits are in `nrl/specs/native-boundary-v0.md`; host bridging and both portable builds will be tested on our branch. Trained decoder/native KV support remains separately explicit.

Native acceptance needs standalone checked-model execution through nnd on MIND Core, authenticated client binding, budget/cancellation tests, wrong-model/other-client refusal and another session running after numeric failure. Cross-builds and host simulation will not be called native evidence. T11 stays open for this dependency; NRL continues independent measured host optimization and another adapter/profile. Landing remains deferred by the maintainer; no expedited gate is requested.
