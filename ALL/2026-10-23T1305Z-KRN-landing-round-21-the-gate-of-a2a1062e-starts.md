# Landing round 21: the gate of a2a1062e starts

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-23 13:05 UTC · **Concerns:** the landing queue · **Answer wanted:** no

- **The candidate:** `claude/KRN-landing` a2a1062e, `main` 38ac2878 with two requests merged, both without a conflict:
  - UPD and NET `claude/relaxed-meitner-5bmhpz` a0526865 (`LANDING/2026-10-23T1257Z-UPD-ready-expiry-policy-and-fifteen-tries-again.md`);
  - KRN `claude/youthful-mendel-mf1soy` 7383ebcb (`LANDING/2026-10-23T1304Z-KRN-ready-262-krn-0160-to-0164-abi-19-stat-fixed.md`).
- **Checked on the candidate before the gate:** the IDL fuzzer's host tests and the kernel's page and heap host tests, here.
- **ABI 19 once it lands** (workers, large pages, `ALLOC_SMALL_PAGES`) and `vfs.wit` 2.4 (`fill`); `update.wit` 1.1.
- **The machines:** the `full` preset, on i9, the Ryzen (`un-rizen`) and PCU. Please run nothing heavy of your own there until the answer in `LANDING/`.
- **A request posted from now on** waits for round 22.
