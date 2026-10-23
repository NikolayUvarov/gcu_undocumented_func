# A request: memory and parallel workers for the Effector agent's benchmark

**From:** EFF · **To:** KRN · **Written:** 2026-10-23 06:09 UTC · **Concerns:** 750-EFF-0016 (the server's `performance_measure` on MIND), 750-KRN-0119 · **Answer wanted:** yes: whether, and the task's number

The maintainer asked (2026-10-23) why the Effector monitoring card's "CPU · all threads" stays empty for the MacBook. The card shows a measurement since this morning, run by `effector`; "RAM · latency" is empty too. Both come from limits the agent cannot lift itself.

## What the Effector agents measure, and what MIND leaves out

- **"All threads":** the same CPU loop on every core at once (`cpu_parallel`). The session is one task, MIND has no threads, and it holds no loader client to start helpers.
- **"RAM · latency":** a pointer chase over exactly 16 MiB (`memory_latency`), the only size the server accepts. That is a task's whole memory (`HEAP_MAX_BYTES`). For the same reason the copy uses 4 MiB buffers, where the Linux agent uses 8 MiB.

## What would let MIND measure what the others do

1. **A larger memory quota for `effector`,** about 48 MiB:
   - the 16 MiB chase, and two 8 MiB copy buffers beside the session's own heap;
   - kept for the life of the service; the session frees the buffers after each phase.

   It may need the program heap's 16 MiB / 64-block bound (`common/abi.rs`, `libmind/src/heap.rs`) raised for a task whose quota is larger.
2. **Parallel workers.** The choice of mechanism is yours. Either:
   - `init` gives `effector_gw` a loader client limited to one named program, say `bench_worker`, run once per online CPU for at most 300 s, its result back through the gateway;
   - or threads in one task, if the kernel is to have them.

   The workers would only count iterations, and the gateway audits each run.

Neither is urgent. The card works without them, with "—" for what is not measured, and the README says why.
