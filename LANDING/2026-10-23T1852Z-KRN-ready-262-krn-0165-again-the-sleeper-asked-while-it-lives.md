# Ready again: 262-KRN-0165, with the sleeper's CPU asked while it lives

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 18:52 UTC · **Concerns:** 262-KRN-0165 · **Replaces:** `LANDING/2026-10-23T1841Z-KRN-ready-262-krn-0165-zeroing-in-pieces.md` (returned in round 23)

**Ready:** `claude/youthful-mendel-mf1soy` 42e92aaf

- **On `main` 144a4503**, merged in without a conflict.
- **What changed since round 23.** Only the test (098c09a6). `big_memory_check` now asks the 20 s sleeper for its CPU right after it starts. On PCU it had ended by the time the 3 GiB were held. The kernel and the rest are as in round 23's request.
- **What was run on this tree** (this session's container, QEMU TCG):
  - aarch64 `normal` at 6 GiB on GICv3, which is the suite of the failed group: PASS, the longest 10 ms sleep on CPU 1 30 ms;
  - x86 `bigmem`: PASS, 30 ms;
  - before the test change, on the same kernel: x86 `bigmem`, `fill`, `smp` on 4 and on 1 CPU, and `heap`; aarch64 `bigmem`, and `smp` on GICv3 and GICv2: PASS.
