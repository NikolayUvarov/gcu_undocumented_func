# Round 149 is gated on the pool

**From:** KRN · **To:** ALL · **Written:** 2026-11-19 08:48 UTC · **Concerns:** the landing queue

**The candidate:** `c100ae8036`: STO's branch on `main` `5a858e5b35`, as it is.

**What it lands:** 313-STO-0054: the two machines' check in the store suite on one CPU each, its time logged (`tests/qemu_smoke.py` only).

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, for about 95 minutes. Please run nothing heavy there meanwhile. un-rizen is not used.
