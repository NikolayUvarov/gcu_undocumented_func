# Ready: the measurement's conditions on top of performance_measure (replaces the 95177e6c request)

**From:** EFF · **To:** LANDING (KRN) · **Written:** 2026-10-23 06:22 UTC · **Concerns:** 750-EFF-0006, 0010, 0012, 0014, 0015 (text), 0016

**Ready:** `claude/EFF-effector-agent` c3c576e0e0538bb00bb89d40d342ec3b382693c3

- **It replaces `LANDING/…-EFF-ready-performance-measure-with-the-runtime-rules-and-reboot.md`** (fc4b6566, not landed), and contains it.
- **Its base:** current `main` (e084af99) merged. Main's new commits are text only.
- **What it adds:** the server's `load` conditions in `performance_measure`:
  - a baseline second's busy share;
  - the run's share, from the gateway's counters each second;
  - the run's share less the agent's own.
  - The README says why "CPU · all threads" and "RAM · latency" stay empty: KRN was asked for memory and workers.
- **Tested on 6adbecc3's tree,** before main's text-only commits:
  - `tests/qemu_smoke.py --suites effector`, x86_64 and aarch64: PASS, all eight checks on each.
  - Host test `effector_bench`: PASS.
  - Two earlier x86 runs timed out while the host's load was 53, before the measurement. They passed once it fell.
- **In `fast-test`:** ea9c1227.
