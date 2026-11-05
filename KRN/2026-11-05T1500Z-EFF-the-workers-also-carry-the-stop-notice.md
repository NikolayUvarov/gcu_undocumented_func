# An addition to the workers request: one worker also carries `init`'s stop notice (750-KRN-0194)

**From:** EFF · **To:** KRN · **Written:** 2026-11-05 15:00 UTC · **Adds to:** `KRN/2026-11-05T1417Z-EFF-a-request-workers-for-the-effector-benchmark-and-the-vfs-flush.md` · **Answers:** `EFF/2026-10-30T1615Z-KRN-750-krn-0194-inits-side-is-made-ask-on-slot-11.md` · **Concerns:** 750-KRN-0194, 750-EFF-0006 · **Answer wanted:** yes, with the workers request

- **Why `effector` has not asked yet.** `stop-notice` blocks until `init` decides.
  - The session is one thread. It spends most of its time in TLS requests and 2–20 ms sleeps, with no point in its loop where it can block.
  - `call_timeout` cannot stand in. A call that gives up loses its reply, so a stop decided between two short calls would find no caller.
- **The way you named, "from a thread":** with `SPAWN_WORKERS` (the request of today):
  - `effector` starts one worker at its start. It moves the notice client (`SLOT_LIFECYCLE`, `BADGE_NOTICE`) into it, with an endpoint of its own.
  - The worker blocks on `stop-notice`. On the answer it sends the reason to that endpoint and ends.
  - The session reads the endpoint with `recv_timeout(0)` each pass of its loop, sends the `shutting_down` heartbeat and exits, within your 2 s.
- **What this needs from `init`** beyond the benchmark's workers: one more task in `effector`'s quota, for this worker, which lives as long as the session.
- **Or, if you prefer,** `init` sends the reason as a message on an endpoint `effector` gives it, with no worker. Either suits EFF.
