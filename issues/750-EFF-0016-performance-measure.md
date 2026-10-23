# 750-EFF-0016 — The server's performance measurement on MIND Core

**Type:** session · **Owner:** `EFF` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-10.2, MC-12.1, MC-12.2

## Problem

- **What the maintainer saw (2026-10-23).** The monitoring card's "Замерить" answered `denied · action_not_allowed` on the MacBook.
- **What the server sends.** `performance_measure` runs the agents' fixed benchmarks for about a minute (Effector's `agent/linux/performance_benchmark.go`). It keeps the result its `AcceptPerformance` takes (`server/telemetry_store.go`).

## Plan

1. **The same algorithms,** so the numbers compare (`effector/src/bench.rs`):
   - `effector_bogomips_v1` on one thread, for a third of the duration;
   - `effector_memcpy_v1`, a twelfth, between two 4 MiB buffers (8 MiB on Linux: a task has 16 MiB in all, `HEAP_MAX_BYTES`);
   - `effector_seqio_v1` writes and reads, a twelfth each, in `system/effector/bench`.
2. **A slice at a time between heartbeats** (200 ms). The agent stays online and reports itself busy with the command. Each rate counts the time its slices ran.
3. **The result is kept** in `system/effector/result` and journaled as `reported` before its ACK.
4. **Left out,** all optional to the server:
   - the pointer chase, which the server takes only over exactly 16 MiB, a task's whole memory;
   - the parallel CPU phase, which needs threads the session has not;
   - the load sampling.

## Acceptance criteria

- [x] Host tests (`tests/effector_bench_host.rs`): the CPU loop is the Linux agent's; a run in slices gives a result with every field the server checks; a failed disk leaves the disk numbers out.
- [ ] The `effector` QEMU suite on x86_64 and aarch64: a 12 s run accepted with every field, heartbeats under 5 s apart meanwhile.
- [ ] On the MacBook: the card shows a saved measurement.

## Related

[750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0014](750-EFF-0014-telemetry.md), `docs/effector/protocol.md` 1.2.
