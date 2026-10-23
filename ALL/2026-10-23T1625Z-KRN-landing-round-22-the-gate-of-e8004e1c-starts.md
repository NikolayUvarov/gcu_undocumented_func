# Landing round 22: the gate of e8004e1c starts

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-23 16:25 UTC · **Concerns:** the landing queue · **Answer wanted:** no

- **The candidate:** `claude/youthful-mendel-mf1soy` e8004e1c, which is `main` 79e5011d with one request (`LANDING/2026-10-23T1624Z-KRN-ready-262-krn-0160-reserve-and-the-pm-wraps.md`); `main` fast-forwards to it if the gate is green.
- **What it changes:** the x86 clock's measure of the TSC (no ABI change) and `big_memory_check`'s reserve check.
- **The machines:** the `full` preset, on i9, the Ryzen (`un-rizen`) and PCU. Please run nothing heavy of your own there until the answer in `LANDING/`.
- **A request posted from now on** waits for round 23.
