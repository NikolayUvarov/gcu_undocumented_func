# Ready: 308-STO-0022's store side, the volume key kept by `keystore` (key source 2)

**From:** STO · **To:** LANDING · **Written:** 2026-10-29

**Ready:** `claude/relaxed-meitner-5bmhpz` dbadf1e9

On `main` 26f14525, merged in. Nothing but text changed since 3d7e2e95, the commit tested. The branch's code is in `fast-test` (d4cf5027, 3d7e2e95's test line aside).

## What it holds (8bf9fe31, 3d7e2e95)

- **Key source 2, kept** (`blockstore/src/store.rs`): the superblock holds an 8-byte id and the check, never the key. The key is the store's secret `volume-<id in hex>`, put and got through `keystore.wit` 1.4's `secret-put` and `secret-get` on a client in the store's slot 3, as 308-KRN-0183 plans it.
- **Which media.**
  - A RAM disk keeps its key in its superblock, as nothing of it outlives a reset.
  - Any other medium's key is kept when it is random and `keystore` keeps it.
  - A derived key, or one `keystore` refuses, stays in the clear, and the log says why.
  - No conversion of a medium already formatted.
- **Refused at mount:** a kept key's medium without the client or its secret (`key` on the wire, the secret named in the log); another key under the id.
- **On the platform nothing changes until 308-KRN-0183:** no slot 3 is lent, so every medium keeps its key in the clear and says `NO SECRETS CLIENT`. Landing it now lets 0183 be checked against `main`.
- **Files of other tracks' areas:** `tests/checkpoint_host.rs` (the mount's new argument), and a docstring in `scripts/make_usb_image.py`.

## Tested on 3d7e2e95

| Where | What passed |
|---|---|
| This session's container (x86) | `scripts/host_tests.sh` whole (exit 0); the store's tests are 61, four of them new |
| This session's container (x86, QEMU TCG) | `store` (a RAM disk says its key stays in its superblock), `storefaults`, `boot`, `vfs`: 37 PASS, exit 0; one SKIP, the updater's stand-in not built |
| `un-rizen_2a8b108d5593` through Effector (aarch64 under TCG) | build; `store` (with memory domains, on `main`'s `init`), `storefaults`, `vfs` |

## Advances

308-STO-0022: its QEMU criteria wait for 308-KRN-0183. Closes nothing.
