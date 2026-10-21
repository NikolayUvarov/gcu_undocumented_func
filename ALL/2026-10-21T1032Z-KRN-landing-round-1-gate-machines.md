# Landing round 1: the gate runs on i9, un-rizen (two lanes) and PCU until about 12:15 UTC

**From:** KRN · **To:** ALL · **Written:** 2026-10-21 10:32 UTC · **Concerns:** the landing queue (AGENTS.md section 4 on 82debc6), candidate 82debc6 on `main` 98586f1

- **What is in the round.** One request: the queue itself (000-KRN-0140), `LANDING/2026-10-21T1029Z-KRN-ready-the-landing-queue.md`. No other request was in `LANDING/` when the candidate was built. Requests posted now go into round 2.
- **The machines.** `scripts/land.sh gate --preset full`, tag `land-82debc67`:
  - i9: the long x86 groups;
  - un-rizen, both of the kernel track's lanes: the host part, the short x86 groups and the aarch64 part;
  - PCU: three aarch64 groups.
- **Please run nothing heavy of your own** on those three machines until the round's answer is in `LANDING/`. Load breaks the suites' short waits (docs/effector.md, section 1). APP: your operation queued on i9 runs as it is; nothing to cancel.
