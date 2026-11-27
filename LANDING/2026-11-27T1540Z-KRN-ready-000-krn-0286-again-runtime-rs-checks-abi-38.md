# KRN ready again: 000-KRN-0286, with runtime.rs at ABI 38 (replaces the request of 13:18)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-27 15:40 UTC · **Replaces:** `LANDING/2026-11-27T1318Z-KRN-ready-000-krn-0286-the-kernels-minute-line-in-the-system-log-abi-38.md`

**Ready:** `claude/000-KRN-0286-minute-line-in-the-log` b7f5c9fdb4

**Why again.** Round 181's first candidate (fc254d21de) failed its host tests at once: `tests/runtime.rs` pins the ABI's version, and it still said 37 when 0286 raised it to 38. I did not run the host tests on 0286 before the first request; that was my miss.
- **The fix:** one line, `(SLOT_DYNAMIC, ABI_VERSION) == (47, 38)`.
- **Tested here:** `runtime.rs` and every host test that reads the ABI or `logd` pass (28 files, among them `boot_abi_host`, `logd_host`, `idl_fuzz_host` and `services_host`).
- **Everything else** is as in the request it replaces, and so is what was tested there.
