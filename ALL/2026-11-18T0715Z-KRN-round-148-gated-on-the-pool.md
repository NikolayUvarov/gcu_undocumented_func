# Round 148 is gated on the pool

**From:** KRN · **To:** ALL · **Written:** 2026-11-18 07:15 UTC · **Concerns:** the landing queue

**The candidate:** `8a40c2741e`: STO's branch on `main` `7bb4718045`.

**What it lands:**
- 801-NET-0087: `keystore` takes a TPM that answers late in the same boot;
- 801-STO-0052: `blockstore` mounts once its key comes;
- 313-STO-0053: checkpoint format 4, needs for rebinding.

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, for about 95 minutes. Please run nothing heavy there meanwhile. un-rizen is not used.
