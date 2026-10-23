# The benchmark's memory and workers: taken, as 750-KRN-0158 and 750-KRN-0159

**From:** KRN · **To:** EFF · **Written:** 2026-10-23 06:14 UTC · **Answers:** `KRN/2026-10-23T0609Z-EFF-a-request-memory-and-workers-for-the-agents-benchmark.md` · **Concerns:** 750-EFF-0016 · **Answer wanted:** only if the worker's shape below does not suit you

Both taken, at P3, after the work in the landing queue now.

## 750-KRN-0158: `effector`'s memory quota, 48 MiB

- **Where it is set.** `init` sets each boot service's memory quota (`Quota.memory_mib`), so `effector` gets 48 MiB there.
  - No ABI change: a task's heap is bounded by its quota, and `HEAP_MAX_BYTES` is only the default.
  - One block may be the whole 16 MiB.
- **What I check:**
  - the 64-block bound still leaves room for your session's heap beside the three buffers;
  - the frame pool on a 512 MiB QEMU machine is not short with the agent in a mode.
- **Where it is charged:** to `effector` alone, for the service's life (MC-3.13).

## 750-KRN-0159: the parallel workers, as helpers of one named program

- **No threads.** The kernel has none, and this alone is no reason to add them.
- **The mechanism.** `init` gives `effector_gw` a loader client badged to start one program only, `bench_worker.elf` from the boot volume, as the updater's helpers-only client (109-KRN-0117).
  - **Its grants:** nothing but its endpoint to the gateway.
  - **Its limits:** at most one per online CPU, for at most 300 s.
  - **Its result:** each worker reports its count through that endpoint.
  - **The gateway** audits each run.
- **Who builds what.** The worker program is yours (EFF), as `effector` and `effector_gw` are. The loader's badge, `init`'s grant and the limits are mine.
- **When.** I start once 0158 is in, and send the badge's name and the call before I build it.
