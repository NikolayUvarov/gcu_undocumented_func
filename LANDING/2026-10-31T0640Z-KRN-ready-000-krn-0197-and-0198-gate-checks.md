# KRN landing request: 000-KRN-0197 (the aarch64 idle check by thread) and 000-KRN-0198 (two waits for a loaded host)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-31 06:40 UTC · **Concerns:** 000-KRN-0197 (made), 000-KRN-0198 (made); closes 000-KRN-0129 and 000-NET-0036 once landed

**Ready:** `claude/youthful-mendel-mf1soy` f47e13fe62b2b1bf1f4cbe7d486575299094ca4c

`main` 43eafe4b is merged.

## What it holds (tests and the landing tool only; no kernel, service or ABI change)

- **000-KRN-0197, the aarch64 idle check** (`tests/qemu_smoke.py`), for round 62's first failure (PCU, GICv2, 0.85 s), and rounds 45 and 60:
  - **Before:** QEMU's processor time over a second, against 0.85 s.
  - **Now:** the share of a second QEMU's busiest thread ran or waited to run, from `/proc/<qemu>/task/*/schedstat`, the lowest of three samples, against 0.9. A vCPU that spins instead of waiting in WFI keeps its thread runnable all of it on any host.
  - **The suites' lines** carry the figure: `WFI (a thread runnable N%)`.
  - **`scripts/land.py`:** `NOT_ON` goes, so PCU takes "aarch64: 16 CPUs" again. `tests/land_test.py` expects that.
  - **`docs/profile/aarch64/evidence.md`** names the new measure.
- **000-KRN-0198, two waits of the x86 suites**, for round 62's other two first failures (un-rizen):
  - the `boot` suite waits 120 s for the loader's start and 60 s for its error, which was 30 s from QEMU's start;
  - the `vfs` suite waits 30 s for the prompt after `run record -t 3 data/rec.avi &` (8 s before), while `vfs_server` may still be writing the last recording.

## Tested

- **Mutation, local** (container, aarch64 `normal` suite): with `nop` for `wfi` the check failed at 100%; without it, it passed at 15%.
- **PCU through Effector** (Ubuntu 22.04, QEMU 8.2.2, TCG), cf0161af:
  - "aarch64: GICv2 with GICv2m" PASS, at 46% and 45%;
  - "aarch64: 16 CPUs" PASS five times in a row, at 40% to 42%. This is 000-KRN-0129's and 000-NET-0036's last criterion.
- **un-rizen through Effector**, f4c42ea3: GICv2 PASS (14%, 15%), "16 CPUs" PASS (37%, under a load average of 58).
- **x86 through Effector:**
  - acc-1, d40bac34: "x86: RAM above 4 GiB" PASS in 955 s, and "x86: boot, display, shell, memory, clock" PASS in 698 s;
  - un-rizen's second lane, cf0161af: both PASS, in 1000 s and 688 s.
- **Host:** `tests/land_test.py` passes.
