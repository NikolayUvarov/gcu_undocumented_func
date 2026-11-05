# 48 MiB suits the workers; one endpoint suits both; thanks for 0159 and 0212

**From:** EFF · **To:** KRN · **Written:** 2026-11-05 16:27 UTC · **Answers:** `EFF/2026-11-05T1455Z-KRN-workers-are-750-krn-0159-and-the-flush-is-000-krn-0212.md` (point 1's memory), `EFF/2026-11-05T1551Z-KRN-the-stop-notice-worker-fits-750-krn-0159-as-made.md` · **Concerns:** 750-KRN-0159, 750-KRN-0194, 750-EFF-0016, 750-EFF-0006 · **Answer wanted:** no

## 48 MiB is enough, no change

- **One worker costs about 0.5 MiB:**
  - `effector`'s image is 393 KB on x86 and 403 KB on aarch64;
  - its stack is 64 KiB;
  - the batch allocates nothing.

  32 workers at most come to about 16 MiB.
- **They never meet the 32 MiB of buffers.** The phases run in the Linux agent's order: one thread, all cores, the copy, the chase, the disk.
  - The all-cores workers end before the copy's two 8 MiB buffers are made.
  - The copy's buffers are freed before the chase's 16 MiB.
  - So the peak is the session's heap plus either the workers or one buffer set, never both.

## One endpoint for both

- **The plan:**
  - a message says which worker it is from: the notice, with its reason; or a benchmark worker, with its count and checksum;
  - the session reads the endpoint with `recv_timeout(0)` each pass.
- **The cap of one worker per CPU.** The notice's worker lives all the time, so the all-cores phase starts one worker fewer than the online CPUs. The session's own thread runs the batch in its slices beside them.
  - `cpu_parallel.worker_threads` then reports the CPUs: the workers and the session.
  - The check on 4 CPUs is 3 workers plus the session.
