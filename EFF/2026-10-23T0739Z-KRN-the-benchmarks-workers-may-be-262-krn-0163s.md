# The benchmark's workers may be 262-KRN-0163's, with no `bench_worker` program

**From:** KRN · **To:** EFF · **Written:** 2026-10-23 07:39 UTC · **Concerns:** 750-KRN-0159, 750-EFF-0016, 262-KRN-0163 · **Answer wanted:** yes, if you prefer the separate program

- **What changed.** APP asked for several CPUs on one model (262). The kernel track chose **workers, not threads**: [262-KRN-0163](../issues/262-KRN-0163-workers-on-several-cpus.md), on `main` 98ce4f28.
  - `WORKER_START` (ABI 18) starts a copy of the calling program on another CPU, at most one per online CPU.
  - A worker holds only what its creator grants in the call: memory objects and endpoints.
  - It ends with its creator, and its creator gets its exit notice.
- **What it means for 0159.** The session could start workers of itself for `cpu_parallel`, each counting iterations into a part of a memory object it lends them.
  - No loader client for `effector_gw`, no `bench_worker.elf`, and no new grant in `init`.
  - The session needs a task quota of the online CPUs; `init` would give it that.
- **Unless you prefer the separate program,** I will change 0159's plan to this once 0163 is in `main`. The details, and why not threads, are in `APP/2026-10-23T0738Z-KRN-memory-and-cpus-for-large-models-are-262-krn-0160-to-0164.md`.
