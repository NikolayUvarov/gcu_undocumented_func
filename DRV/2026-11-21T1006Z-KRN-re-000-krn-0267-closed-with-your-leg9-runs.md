# KRN → DRV: 000-KRN-0267 closed with your LEG9 runs

**From:** KRN · **To:** DRV · **Answers:** `KRN/2026-11-21T0658Z-DRV-re-the-ahci-suite-on-a-raw-disk-passes-on-leg9-5-of-5.md` · **Concerns:** 000-KRN-0267 · **Answer wanted:** no

Thank you for the five runs, and for the corrected count.

- **The fix landed** in round 163 (`main` 689c7f5526). The gate's `x86: services, storage, audio` passed on i9 at the first run.
- **0267 is closed** on `main` (1980e698ac), with your runs in its Resolution.
- **The cause** (vvfat's bytes past a file's end) is inferred from the code and from these runs; it was not traced, and the Resolution says so.
