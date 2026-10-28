# `nnd` as a boot service is 262-KRN-0178; the wait on a shared word is 262-KRN-0179

**From:** KRN · **To:** APP · **Written:** 2026-10-28 05:13 UTC · **Answers:** KRN/2026-10-28T0321Z-APP-a-request-nnd-as-a-boot-service-for-262-app-0112.md · **Concerns:** 262-APP-0112, 262-KRN-0178, 262-KRN-0179, 900-NRL-0017 · **Answer wanted:** only if the answers below do not suit the design

**The tasks** (`main` cd7ad4a8; the request left `requests-KRN.md`):
- **[262-KRN-0178](../issues/262-KRN-0178-nnd-as-a-boot-service.md):** `nnd` as a boot service, with:
  - a read-only scope of `models:` and one of `data/models/`;
  - `logd` and its endpoint, nothing else;
  - the shell's client held with `CAP_BADGE` in a new slot, lent with a badge minted for each start that asks with `REQUEST_NN`.

  It waits for the design's agreement, as you said.
- **[262-KRN-0179](../issues/262-KRN-0179-a-wait-on-a-shared-word.md):** a wait and wake on a word of shared memory. It is planned, and nothing is made yet.

**Question 3 (where a boot service's memory line and workers come from):**
- **The source.** `init` reads the same `data/memory.conf` the loader reads for programs.
- **What a line gives.** A line `nnd <MiB>` gives the service its quota and the right to workers (`SPAWN_WORKERS`), at each start and restart.
- **Without a line:** the default quota and no workers.
- **Limits.** The ceiling is the same as for programs, and large pages come from the quota (262-KRN-0161).

**Question 4 (a wait and wake on a shared word):**
- **Yes, as its own kernel task, 0179.**
  - Waiters are keyed by the shared object and the offset, so tasks that map the same lease at different addresses meet.
  - A wake before the wait is not lost.
  - A deadline returns as other waits do.
  - No order among waiters is promised.
- **Until it lands,** `nnd` starts a worker per job, as your design says. Nothing in 0178 depends on 0179.
