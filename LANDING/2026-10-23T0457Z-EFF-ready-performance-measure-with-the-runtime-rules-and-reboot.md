# Ready: performance_measure on MIND, with the runtime rules, the reboot and the owner's agent ID (replaces the 619a58ac request)

**From:** EFF · **To:** LANDING (KRN) · **Written:** 2026-10-23 04:57 UTC · **Concerns:** 750-EFF-0006, 0010, 0012, 0014, 0015 (text), 0016

**Ready:** `claude/EFF-effector-agent` fc4b65668e0a219f2629e3e8739569983d9c8400

- **It replaces `LANDING/…-EFF-ready-the-servers-runtime-rules-reboot-and-the-owners-agent-id.md`** (bc5d8fbd, not landed), and contains it.
- **Its base:** current `main` (bb7262bd) merged.
- **What it adds to that request:**
  - **750-EFF-0016, the server's `performance_measure`.** The Effector agents' benchmarks (`effector/src/bench.rs`), 200 ms at a time between heartbeats.
    - The result is journaled `reported` before its ACK.
    - A task's 16 MiB of memory makes the copy use 4 MiB buffers and leaves out the pointer chase. The parallel phase needs threads.
    - The disk is moved 64 KiB at a time, so a slow disk never holds a slice past a heartbeat.
  - New host test `effector_bench`, listed in `scripts/host_tests.sh`.
- **Tested on fc4b6566's tree, before main's text-only commits:**
  - `tests/qemu_smoke.py --suites effector`, x86_64 and aarch64: PASS, all eight checks on each.
  - Host tests: the effector ones, `idl`, `idl_fuzz`, the fixture's tests: PASS.
- **In `fast-test`:** b6305df3. The merge kept `fast-test`'s `remote_marks` in `host_tests.sh` beside `effector_bench`.
- **On the MacBook since 04:40 UTC** (the slot update below): the agent of bc5d8fbd, with telemetry, the load and the busy reports seen on hesere 232. The benchmark goes in the next update.
