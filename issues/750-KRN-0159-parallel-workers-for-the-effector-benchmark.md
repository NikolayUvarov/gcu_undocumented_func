# 750-KRN-0159 — Parallel workers for the Effector agent's benchmark

**Type:** `init`, `loader` · **Owner:** kernel session · **Priority:** P3 · **Status:** open · **Blocked by:** EFF's `bench_worker` program · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.13, MC-10.2

From EFF's request of 2026-10-23 (`KRN/2026-10-23T0609Z-EFF-a-request-memory-and-workers-for-the-agents-benchmark.md` on `internal-dev-messages`), for 750-EFF-0016.

## Problem

- **What the server measures.** For "CPU · all threads" it runs the same loop on every core at once.
- **Why MIND cannot.** The kernel has no threads, and the Effector services hold no loader client to start helpers.

## Plan

1. **No threads.** The kernel has none, and this alone is no reason to add them.
2. **A loader client for one named program.** `init` gives `effector_gw` a loader client badged to start only `bench_worker.elf` from the boot volume, as the updater's helpers-only client (109-KRN-0117).
   - **A worker's grants:** only its endpoint to the gateway.
   - **Its limits:** at most one per online CPU, each for at most 300 s.
   - **Its result:** it reports its count through that endpoint.
3. **The gateway audits each run** (EFF).
4. **Who builds what.** The worker program is EFF's. The badge, `loader`'s rule, `init`'s grant and the limits are the kernel track's.

## Acceptance criteria

- [ ] In the `modes` suite (x86 and aarch64), the gateway's stand-in starts one worker per CPU through the client, and the loader refuses any other program and a worker past the limits.
- [ ] `docs/profile/bootstrap.md` names the grant.

## Related

[750-KRN-0158](750-KRN-0158-the-effector-sessions-memory-for-its-benchmark.md), 109-KRN-0117 (the helpers-only loader client).
