# 750-KRN-0159 — Parallel workers for the Effector agent's benchmark

**Type:** `init` · **Owner:** kernel session · **Priority:** P1 (the maintainer, 2026-11-05) · **Status:** in progress · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.13, MC-10.2

From EFF's request of 2026-10-23 (`KRN/2026-10-23T0609Z-EFF-a-request-memory-and-workers-for-the-agents-benchmark.md` on `internal-dev-messages`), for 750-EFF-0016, and its new request of 2026-11-05 (`KRN/2026-11-05T1400Z-EFF-…`).

## Problem

- **What the server measures.** For "CPU · all threads" it runs the same loop on every core at once.
- **Why MIND does not.** On the Ryzen the server shows `worker_threads: 1` of 16 and no `cpu_parallel`. `effector` measures on its own thread: workers (262-KRN-0163, ABI 18) are in `main`, but `init` starts `effector` without `SPAWN_WORKERS`. Only `nnd` gets it.

## Plan

Revised 2026-11-05 at EFF's request. Workers replace the earlier plan of a second program, `bench_worker.elf`, started through a loader client: they need no new badge, rule or program.

1. **`init` starts `effector` with `SPAWN_WORKERS`,** in the modes where it runs now, with:
   - a task quota of 32. It is a ceiling: the kernel lets one worker live per online CPU, and `init` starts before the other CPUs and cannot count them, as for `nnd`;
   - an endpoint quota of 1, for the endpoint its workers answer on;
   - its 48 MiB memory quota unchanged (750-KRN-0158). Each worker's image, stack and memory are charged to it.
2. **A worker holds only what `effector` grants it.** EFF's design is one endpoint that `effector` made. The kernel never lets a worker hold a privilege.
3. **EFF's part (750-EFF-0016):** start the workers, collect their answers and send `cpu_parallel`. A start refused leaves `cpu_parallel` out.

## Acceptance criteria

- [x] QEMU, x86 and aarch64 (the Effector suite): `effector`'s quota is 32 tasks, 1 endpoint and 48 MiB, and the capabilities it holds are as before.
- [x] `docs/profile/bootstrap.md` names the right.
- [ ] EFF's check: on 4 CPUs, `cpu_parallel.worker_threads` is 4.

## Progress

2026-11-05, `init`'s part, on `claude/750-KRN-0159-effector-workers`:
- `effector` starts with `SPAWN_WORKERS` and the quota of point 1 (`EFFECTOR_TASKS`). `init` logs `[INIT] effector: 48 MIB, 1 ENDPOINT AND WORKERS, AT MOST ONE PER CPU`.
- The `modes` suite's `effector_grants_check` requires that quota and that line.
- `docs/profile/bootstrap.md` names the right.
- **Tested** on 84bfcf4c through Effector, the `modes` suite with its check:
  - acc-1, x86: `services, storage, audio` passed;
  - acc-2, aarch64: `the updater and the modes` passed when run again. The first run failed before `modes`, in `update_suite` at `VERSION FLOOR 1 RAISED TO 6`, a race of that suite that 000-KRN-0210 fixes.

2026-11-05, landed in round 86 (`main` 6253e7a7). Its gate passed the `modes` suite's `effector_grants_check` on x86 (`services, storage, audio`) and aarch64 (`the updater and the modes`), and the Effector suite on both.
- EFF (`KRN/2026-11-05T1627Z-EFF-…` on `internal-dev-messages`): 48 MiB suits the workers, and one endpoint suits both the stop notice's worker and the benchmark's.

**Left:** the third criterion, EFF's check of `cpu_parallel.worker_threads` on 4 CPUs (750-EFF-0016).

## Related

[750-KRN-0158](750-KRN-0158-the-effector-sessions-memory-for-its-benchmark.md), [262-KRN-0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done) (workers).
