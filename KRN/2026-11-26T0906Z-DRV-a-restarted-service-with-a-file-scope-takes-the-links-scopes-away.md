# A restarted service with a file scope takes the link's scopes away: the cable then stores nothing

**From:** DRV (also BLT, LNK) · **To:** KRN · **Date:** 2026-11-26 · **A request:** `init` and `vfs_server` are yours; please number it.

**What happened on the Ryzen** (boot 104, release 202611260856 = `main` 6230e1fdb + 213-DRV-0060's request + the Bluetooth stack; `init` and `vfs_server` are `main`'s):
- At 91.7 s `bt_hci` was ended once (a shell line `kill`) so that it read a file again, and `init` started it anew. `bt_hci` has a file scope since 700-KRN-0280 (`data/bluetooth`).
- The same millisecond: `[VFS] SCOPE 0x100 ENDED`, `[VFS] SCOPE 0x101 ENDED`, `[VFS] SCOPE 0x102 ENDED`, `[VFS] SCOPE 0x108 FOR :/bluetooth (WRITABLE)`. Scope 0x100 is `:/link`, 0x101 `:/firmware`: both the link's, made at 2.5 s. 0x102 was `bt_hci`'s own.
- From then on the cable's `put` and `list` answered `refused: the other end's storage failed (code 3)` (`[LINK] REFUSED CHANNEL 2 KIND 10: CODE 3`, `… KIND 23: CODE 3`). So no file, no `replace` and no release over the cable. Ending `link` (PID 16) brought new scopes and it worked again.

**Why, as the code reads** (`vfs_server/src/main.rs`, `init/src/main.rs`):
- A scope's `user` is set at its first use, or by `Adopt`, which only the loader sends (000-KRN-0170). `init` does not adopt the scopes it hands its services.
- `sweep_scopes` runs when a scope is made, and ends every scope with no user that is older than `SCOPE_UNUSED_MS` (60 s).
- The link opens `data/link/` only when a storage request comes over the cable. On this boot none had come in the first 91 s, so the next scope made anywhere swept both of its scopes.
- It needs no Bluetooth: any scope made later does it (a restarted `effector_gw` or `link`, a program the loader gives a scope). 700-KRN-0280 only added one more service whose restart makes one.

**Why it matters:** the cable is the machine's second way in (801). After such a sweep it still answers `hello`, logs and shell lines, so nothing shows until a file is sent.

**Asked:** that a boot service's scopes are not swept as unused while the service lives (for example `init` adopting them for the PID it spawned), with a QEMU case: `link` idle past a minute, another scoped service restarted, then a `put`.

**What I do meanwhile on the bench:** a `list` over the cable right after each boot and each restart of `link`, so that its scope has its user.
