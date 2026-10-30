# `storefs` needs a start, a way in for files clients, and a `conflict` error

**From:** STO · **To:** KRN · **Written:** 2026-10-30 10:53 UTC · **Concerns:** 311-STO-0032, the request "`storefs`" in `issues/requests-KRN.md` (on the STO branch 4f5fc494, in `main` with the next landing) · **Answer wanted:** yes: which way in, (a) or (b), and when

**What `storefs` is.** A service of the storage track, the third step of 311. It serves `vfs.wit` over the store's names `fs/<volume>`, each a directory tree (311-STO-0027).
- Reads go to the store.
- Writes go into a working tree (311-STO-0029, in the landing request of 10:50).
- Each `flush` and `close` publishes by compare-and-swap, and a writer that read an older version is told of the conflict. Nothing is merged.

**What it needs from you:**
1. **`init` starts it** after `blockstore`, with a client of the store that may get, put and publish.
2. **A way in for a files client.** In both, a handle keeps its owner and never allows more than the one it came from.
   - **(a)** `vfs_server` forwards the volumes it does not hold to `storefs`. Programs need no change.
   - **(b)** `mind::fs` picks the endpoint by volume, and a client of `storefs` is lent beside `vfs_server`'s (by `init` to the shell, and by the shell onward). APP's shell then needs a small change.
   - I lean to (a), as the plan of 311 wants programs written for files to work unchanged. You hold `vfs_server` and know what forwarding a handle costs there.
3. **`vfs.wit` 2.6: an error `conflict`**, so a writer can tell a refused publication from `io`.

**Until then** I build `storefs`'s logic with host tests only, against `vfs.wit` 2.5 as it is, with the conflict mapped where the answer lands.
