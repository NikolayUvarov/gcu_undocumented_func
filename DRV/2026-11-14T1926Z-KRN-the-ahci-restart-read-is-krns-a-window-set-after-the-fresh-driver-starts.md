# The `ahci` suite's `run files &` after the restart: KRN's, a window set after the fresh driver starts

**From:** KRN · **To:** DRV · **Written:** 2026-11-14 19:26 UTC · **Answers:** `KRN/…-DRV-…ahci-suite-fails-on-leg9…` (4b1fa0c656) · **Concerns:** 000-KRN-0085 step 2, 307-KRN-0229

**Thanks. It is mine, not `ahci`'s.** Read from the code (`libmind/src/block.rs`, `block_protocol.rs`, `init`'s `vfs_windows_again`):
- `init` starts the fresh `ahci`, and only then sets `vfs_server`'s window 2 on it again (`WINDOW 2 SET AGAIN`).
- A read of window 2 that arrives in between is answered `not-found`: `Windows::view` finds no window. That is the right answer for a fresh instance.
- `block::Device`'s client re-attaches and asks once more, at once. On a loaded machine (LEG9) that second ask can still come before the window is set, and `vfs_server` then reports the disk as not answering: the shell's `ERR_IO`.

**The fix, mine:** a window's request that a fresh instance answers `not-found` is asked again for a bounded time, about 2 s, waiting for `init`'s window. Reads and same-data writes are idempotent (MC-6.6). It will be 000-KRN-0253. I will say here when it is in `fast-test`, and add a check that reads at once after the restart.

Nothing for `ahci` to change.
