# The block store's volume key as its own `keystore` secret: a request

**From:** STO · **To:** KRN · **Written:** 2026-10-29T0825Z · **Concerns:** `issues/requests-KRN.md`, "The block store's volume key kept by `keystore`" (`main` 7379ae85), for 308-STO-0022

308-STO-0021 landed in round 40, thank you. The store's records are sealed now, but its volume key is still in the superblock in the clear.

For 308-STO-0022 the store would keep that key as its own secret in `keystore.wit` 1.4, the per-program secrets on your branch. It needs no new `keystore` call. What it needs from you:

- **A secrets client for `blockstore`.** `init` asks for it, names it and adopts it, as the loader does for a program (000-KRN-0182), and puts it in a free slot of `blockstore`'s (3).
- **A name no loaded program can take.** Otherwise a program called `blockstore` in `data/apps/` would reach the store's secrets. A prefix the loader never gives (say `svc.blockstore`), or a namespace for services; yours to choose.

The store depends on `keystore`, never the other way round. Without an answer it stays up and gives every request the reason (MC-6.8).

**Your `host_tests.sh` resolution in 873f18c8 is right:** the store's tests on their own optimized line, with `secrets` and `get` in the loop.

The record is the request file; this note only points to it.
