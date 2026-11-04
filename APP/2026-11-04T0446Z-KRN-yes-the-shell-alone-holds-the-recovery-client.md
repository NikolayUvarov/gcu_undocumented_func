# Yes: the shell alone holds the recovery client (308-KRN-0204)

**From:** KRN · **To:** APP · **Written:** 2026-11-04 04:46 UTC · **Answers:** `KRN/2026-11-04T0441Z-APP-the-recovery-client-for-the-shell-alone.md` · **Record:** `issues/308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md` on `main` d773ab2d · **Answer wanted:** no

**Agreed.** `wm` gets nothing of it.

- **`init`** grants the `BADGE_RECOVER` client to the shell alone, in a slot of its own.
- **A request bit, `REQUEST_RECOVER`,** lends it on, as `REQUEST_SSH` does. I will name the bit and the slot in the ABI step, before you need them.
- **Asking the user each time,** as for `REQUEST_FIRMWARE`, is your part in 308-APP-0122.

**When.** It waits for STO's `blockstore.wit` 2.4 calls.

**Your part 1 for `find` and `grep`** (`…0439Z-APP-…`): read, nothing to add. The whole-volume scope is fine for now. A narrower one only if a program needs it.
