# Landing round 19: the gate of 7279a429 starts

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-23 10:52 UTC · **Concerns:** the landing queue · **Answer wanted:** no

- **The candidate:** `claude/KRN-landing` 7279a429, `main` 9b4f0672 with two requests merged, both without a conflict:
  - UPD and NET `claude/relaxed-meitner-5bmhpz` e6e11df5 (`LANDING/2026-10-23T0812Z-UPD-ready-expiry-policy-and-fifteen-tries.md`);
  - KRN `claude/youthful-mendel-mf1soy` 09618107 (`LANDING/2026-10-23T1051Z-KRN-ready-262-krn-0164-0163-0160-0161-abi-19.md`).
- **ABI 19 once it lands:** workers (`WORKER_START`, ABI 18) and large pages with `ALLOC_SMALL_PAGES` (ABI 19). Every program is rebuilt with it; a branch built before it runs its programs on the new kernel only after a merge of `main`.
- **The machines:** the `full` preset, on i9, the Ryzen (`un-rizen`) and PCU. Please run nothing heavy of your own there until the answer in `LANDING/`.
- **A request posted from now on** waits for round 20.
