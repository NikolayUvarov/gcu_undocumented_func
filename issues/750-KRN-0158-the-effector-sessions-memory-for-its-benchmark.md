# 750-KRN-0158 — The Effector session's memory quota for its benchmark

**Type:** `init` · **Owner:** kernel session · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.13

From EFF's request of 2026-10-23 (`KRN/2026-10-23T0609Z-EFF-a-request-memory-and-workers-for-the-agents-benchmark.md` on `internal-dev-messages`), for 750-EFF-0016.

## Problem

- **What the server measures.** Its `performance_measure` chases pointers over exactly 16 MiB for "RAM · latency", and copies 8 MiB buffers.
- **Why `effector` cannot.** 16 MiB is a task's whole default quota (`HEAP_MAX_BYTES`), so the session cannot hold the chase beside its own heap. The card stays empty for MIND.

## Plan

- **`init` gives `effector` a memory quota of 48 MiB** (`EFFECTOR_MEMORY_MIB`), as it gives `blockstore` and `windows` theirs.
  - That is room for the 16 MiB chase, two 8 MiB copy buffers and the session's heap.
  - It is charged to the session alone, for the service's life (MC-3.13).
- **No ABI change.** A task's heap is bounded by its quota; `HEAP_MAX_BYTES` is only the default.
- **The session frees the buffers after each phase** (EFF).

## Acceptance criteria

- [ ] In the `modes` suite (x86 and aarch64), `effector` runs with a 48 MiB quota (`stat`), and every other suite passes as before (the gate).
- [ ] EFF's benchmark allocates the 16 MiB chase and the copy buffers in a mode (750-EFF-0016).

## Related

[750-KRN-0159](750-KRN-0159-parallel-workers-for-the-effector-benchmark.md), [750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done).
