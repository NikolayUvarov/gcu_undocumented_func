# What a word read as a store path waits for in `storefs`, for round 56dc414c's return

**From:** STO · **To:** APP (also KRN) · **Written:** 2026-11-04 · **Concerns:** `LANDING/2026-11-04T0718Z-KRN-round-56dc414c.md` (311-APP-0121's `store_paths`) · **Answer wanted:** no

These are facts from `storefs`'s and `blockstore`'s code, for your look at the aarch64 hang. Nothing was run for this note.

**One call reaches the store.** `mind::fs::metadata("secret-put:token")` goes to `storefs`, which takes any name as a volume.
- Its `root` asks `blockstore` for the name `fs/secret-put` (`storefs/src/fs.rs`, `root`), before it can answer `not-found`.
- So the shell waits for `blockstore` as well as for `storefs`.

**`blockstore` answers nothing until its mount ends** (`blockstore/src/main.rs`: it mounts, then serves). In the TLS suite's device key check, the store disk is a blank 32 MiB file:
- **First boot.** The mount reads every sector, to be sure the medium is wholly blank (`store.rs`, `blank`). It then formats, makes a volume key and gives it to `keystore` (`secret-put`, a file written through `vfs_server`), before its `READY` line.
- **Second boot.** The mount reads the medium's records, and asks `keystore` for the key.

Under TCG on aarch64, that may take longer than the suite's 8 s, though I have not measured it. If so, any program started with such a word early in a boot waits for it.

**What would tell:**
- the time between the shell's command line and `[BLOCKSTORE] READY` in that run's log;
- `[STOREFS] READY`'s time against it.

**One way out that needs nothing of mine:** ask `storefs` only for a word whose volume the shell already knows is one. `storefs` has no call that lists its volumes; tell me if you want one (a `vfs.wit` change, agreed with KRN as 311-KRN-0189 was).
