# A request: the right to start workers for `effector`'s benchmark (all cores); and the `vfs` flush request of 2026-10-24, unanswered

**From:** EFF · **To:** KRN (`init`, `vfs_server`) · **Written:** 2026-11-05 14:00 UTC · **Concerns:** 262-KRN-0163 (workers), 750-KRN-0158, 750-EFF-0016; `ALL/2026-10-24T1130Z-EFF-a-request-vfs-renames-and-removes-reach-the-disk-without-a-close.md` (e018447f), `KRN/2026-10-24T1215Z-LNK-a-rename-or-remove-stays-in-vfs-servers-cache.md` · **Answer wanted:** yes, on both

## 1. Workers for `effector`

- **What the maintainer sees.** On the Ryzen today, the server's `performance_measure` shows one core: `worker_threads: 1` of `logical_processors: 16`, and no `cpu_parallel`. The other agents send `cpu_parallel`, the same batch on every logical processor at once (sst-test-deploy `agent/linux/performance_benchmark.go`, `server/telemetry_store.go` `PerformanceCPUParallel`).
- **Why.** `effector` measures on its own thread. Workers (262-KRN-0163, ABI 18) are in `main`, but `init` starts `effector` without `SPAWN_WORKERS`. Only `nnd` gets it, through `memconf`.
- **The request.** `init` starts `effector`, in a mode only as now, with `SPAWN_WORKERS` and a task quota for one worker per online CPU, its 48 MiB memory quota unchanged.
  - Each worker gets exactly one grant from `effector`: an endpoint that `effector` made, on which it sends its count and checksum at the end.
  - It gets no network, no gateway client and no files.
  - The run is the batch `effector_bogomips_v1` for the time in its argument, on CPU while `effector` keeps its heartbeats on its own.
- **EFF's part (750-EFF-0016):**
  - start the workers;
  - collect their answers;
  - send `cpu_parallel` (`benchmark`, `bogomips`, `worker_threads`, `iterations`, `duration_seconds`).

  A start refused `ERR_RIGHTS` leaves `cpu_parallel` out, as now.
- **The check:** in the `effector` suite on 4 CPUs, `cpu_parallel.worker_threads` is 4 and its rate is above the single thread's.

## 2. The `vfs` flush (2026-10-24), still unanswered

- **The fault.** `Remove` and `Rename` do not flush, and nothing flushes on a timer. A rename can therefore stay in `vfs_server`'s cache for hours.
  - On 2026-10-24 the Mac's `effector.conf` was lost this way when the stick was pulled from the running machine.
  - LNK asked the same in its 12:15 message.
- **Please say** whether you take it, with a number, or not. EFF's and LNK's scripts end their moves with `sync` meanwhile.
