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
   - the parallel CPU phase, which needs threads the session has not. Asked of KRN, with the memory, on 2026-10-23.
5. **The conditions** (the server's `load`, added 2026-10-23):
   - the machine's busy share in a baseline second;
   - its share over the run, sampled each second from the gateway's counters;
   - that share less the agent's own.

## Acceptance criteria

- [x] Host tests (`tests/effector_bench_host.rs`): the CPU loop is the Linux agent's; a run in slices gives a result with every field the server checks; a failed disk leaves the disk numbers out.
- [x] The `effector` QEMU suite on x86_64 and aarch64: a 12 s run accepted with every field, heartbeats under 5 s apart meanwhile (after the disk moved to 64 KiB pieces).
- [x] On the MacBook (2026-10-23, operation `20261023051204.958726827_000057`): `completed` in 50 s, saved by the server as the endpoint's `performance`: 202.3 million iterations/s on one thread, 1758 MiB/s copied, 3.1 MiB/s written and 33.1 MiB/s read on the boot stick.

## Related

[750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0014](750-EFF-0014-telemetry.md), `docs/effector/protocol.md` 1.2.

## Progress — 2026-10-23, the conditions on the MacBook

- **The run:** the third slot update (version 202610230622), then operation `20261023064407.116524958_000036`, `completed`.
- **The conditions the server keeps:**
  - 41 samples;
  - 6.1% busy in the baseline second;
  - 13.6% over the run;
  - 7.1% less the agent's own;
  - not under load.
- **The numbers:** 202.4 million iterations/s, 1758 MiB/s copied, 2.9/33.4 MiB/s written/read.
- **Still empty:** "CPU · all threads" and "RAM · latency". They wait on KRN (memory and parallel workers, `KRN/…-EFF-a-request-memory-and-workers…`).

## Progress (2026-11-05): the memory as on Linux; all cores still left out

- **What the maintainer saw.** The Ryzen's measurement through hesere today: `worker_threads` 1 of `logical_processors` 16, no `cpu_parallel`, no `memory_latency`.
- **The memory, now as the Linux agent's.** The session has had 48 MiB since KRN's 750-KRN-0158.
  - The copy moves two 8 MiB buffers.
  - A new phase chases `effector_pointer_chase_v1` over exactly 16 MiB, for duration/15, after the copy and before the disk, as on Linux. Its result is `memory_latency`: `working_set_bytes` 16777216, the accesses, ns per access, and the last index as its checksum.
  - The permutation is the Linux agent's, built 65 536 steps at a time between slices, so no slice outlasts a heartbeat.
  - The buffers are reserved without aborting: a phase whose memory cannot be had is left out of the result.
- **Checked on the host** (`tests/effector_bench_host.rs`). The permutation's first entries, and the index after 4 096 and after 40 960 steps, equal what the Linux agent's own code gives (run in Go 1.23 without its clock). A sliced run gives `memory_latency` as the server's `validPerformanceExtras` wants it.
- **The `effector` QEMU suite** checks the field. Not run yet.
- **`cpu_parallel` (all cores) waits for KRN.** `init` starts `effector` without `SPAWN_WORKERS`; asked on internal-dev-messages, 2026-11-05.

## Progress (2026-11-14): every CPU at once

- **`cpu_parallel`, as the Linux agent's.** `init` lets `effector` start workers since 750-KRN-0159: up to 32, one live worker a CPU.
  - The phase runs after the single thread, for duration/3, ending by the wall clock.
  - The session starts one worker fewer than the CPUs; the stop notice's worker counts as one of them. Each runs `effector_bogomips_v1` until the shared deadline and writes its count in a page the session shares.
  - The session's own thread batches in its slices beside them.
  - The rate is the total over the phase's wall time. `worker_threads` is the workers and the session, at most the logical processors.
  - With no worker started the phase is left out, as before. A worker that never ends leaves it out 10 s past its deadline.
- **Host tests:** three fake workers give `cpu_parallel` with `worker_threads` 4; a stuck one leaves it out.
- **The `effector` suite** checks `cpu_parallel` on 2 CPUs or more. Not run yet.
