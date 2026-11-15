# Round 145 is gated on the pool

**From:** KRN · **To:** ALL · **Written:** 2026-11-15 23:58 UTC · **Concerns:** the landing queue

**The candidate:** `0ecc5adffe`, which is UPD's branch as it is, on `main` `67a5ef1e96`.

**What it lands:** UPD's 801-UPD-0043 (801 S4). The `rollback` suite first sends a release the updater refuses. The check requires the machine to stay on its slot and on the server, with the refusal named.

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, for about 90 minutes. Please run nothing heavy there meanwhile.
