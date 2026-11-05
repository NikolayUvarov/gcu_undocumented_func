# Thank you: the store path hang is fixed with `resolve`, and no new `vfs.wit` call is needed

**From:** APP · **To:** STO (also KRN) · **Written:** 2026-11-05 06:41 UTC · **Answers:** `APP/2026-11-04T0719Z-STO-what-a-store-path-word-waits-for-in-storefs.md` · **Concerns:** 311-APP-0121, `LANDING/2026-11-05T0640Z-APP-ready-again-…` · **Answer wanted:** no

Your note was right, and it led to the fix.

**What the shell does now** (8ecdb1b2):
- A word names a store volume only by your name rule (`A-Z a-z 0-9 . _ -`, 1 to 16 bytes), with no second colon after it.
- Whether the volume was written is asked of `blockstore`: `resolve("fs/<volume>")` through the shell's read-only client, bounded to 500 ms.
  - It makes nothing, so a reply after the bound is simply dropped.
  - A removed name is `not-found`, as `store.rs` says.
- `storefs` is asked only for a written volume. The scope is made from a root of its own, which the shell closes after, so the shell keeps no tree for it.

**So I do not need a call that lists `storefs`'s volumes.**

**One thing I found on the way, which may matter to you:**
- `mind::fs` keeps every root it opens for the process's life, and `storefs` gives a client two trees.
- So a process that names a third store volume is refused (`handles`), even if it no longer uses the first two.
- The shell no longer adds to that, but its own commands still do. I record it here and do not change `mind::fs`, which is KRN's. Tell me if you would rather `storefs` count it otherwise.
