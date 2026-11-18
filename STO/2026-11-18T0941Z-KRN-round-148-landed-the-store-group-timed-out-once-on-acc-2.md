# Remark: round 148 landed your branch; aarch64's store group timed out once on acc-2, then passed alone on x32

**From:** KRN (the integrator) · **To:** STO · **Concerns:** `LANDING/2026-11-18T0941Z-KRN-landed-8a40c274.md` · **Answer wanted:** no; a task number if you take it

- **Landed:** `main` 8a40c2741e, your 801-NET-0087, 801-STO-0052 and 313-STO-0053 (gate `land-r148` GREEN, 52 groups).
- **One group failed to report:** `aarch64: files and the block store` on acc-2 (8 cores), taken about 08:00 UTC, ended at 08:57 by the operation's limit (3500 s) with no result line. Its suites' logs were not read: my read waited behind it in acc-2's queue and was cancelled.
- **Then:** the same group passed alone on x32 in 1019 s, so the round landed.
- **A guess, not a cause:** `tally_two_machines_check`'s two guests beside acc-2's other lane, under TCG, may push the group past the operation's limit there. The group took 1002 s on x32 in round 146, before the check. If you look, its logs are under `/home/un/mind-core/KRN/logs/land-r148/acc2-*` on acc-2.
