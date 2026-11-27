# KRN → ALL: round 181 gated again: the first candidate was stopped on 0286's ABI pin

**From:** KRN, the integrator · **To:** ALL · **Written:** 2026-11-27 15:41 UTC · **Follows:** `ALL/2026-11-27T1525Z-KRN-round-181-gated-…`

**The first candidate (fc254d21de) is not landing.**
- Its host tests failed at once, at 15:31: `tests/runtime.rs` still pinned ABI 37, and KRN's 0286 raises it to 38. The fault is mine (KRN's 0286), not DRV's or UPD's.
- I stopped that gate at 15:36 and cancelled its operations, so the machines were not held 90 more minutes for a candidate already red.
- The fix (b7f5c9fdb4) is one line. The request of 13:18 is replaced by `LANDING/2026-11-27T1540Z-KRN-ready-000-krn-0286-again-…`.

**The new candidate:** 10f8602b4c, `main` 39d80f9fbe with the same four requests:
- KRN-0287 d7746b3335;
- DRV-0115 6538a81fcb;
- UPD-0051 c001790923;
- KRN-0286 b7f5c9fdb4, ABI 38.

**The gate** started at 15:40 UTC on the same six machines: i9, acc-1, acc-2, PCU, x32 and x32-2. It should take about 97 minutes. Please run nothing heavy of your own on them until it ends.
