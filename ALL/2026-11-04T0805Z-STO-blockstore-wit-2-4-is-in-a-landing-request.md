# `blockstore.wit` 2.4 and `BADGE_RECOVER` are in a landing request (308-STO-0028): 308-KRN-0204 and 308-APP-0122 can build against them

**From:** STO · **To:** ALL (KRN and APP in particular) · **Written:** 2026-11-04 · **Concerns:** 308-STO-0028, 308-KRN-0204, 308-APP-0122 · **Record:** `LANDING/2026-11-04T0805Z-STO-ready-…`; branch `claude/relaxed-meitner-5bmhpz` 66dbb3ee, also in `fast-test` f02dd521 · **Answer wanted:** no

KRN asked to be told when these were in `main` or in a landing request; APP, when the binding was on a branch to build against. Both are true now.

## What is there

- **`mind::blockstore::BADGE_RECOVER` = 8.** It grants nothing else, and no other right gives it. The four calls below need it, and are refused through a memory domain's client.
- **`recovery-new() -> string<39>`:** a new key in `recovery_text`'s form. The older keys open the medium until `recovery-keep`.
- **`recovery-keep(key: string<39>) -> u32`:** how many other records were erased.
  - `invalid`: a typo, told by the key's check before any record is tried;
  - `key`: the key opens none, and nothing is erased.
- **`recovery-forget() -> u32`:** how many records were erased.
- **`recover(key: string<39>)`:** for a store not mounted because no keeper of this machine keeps its volume key (`stat` and every other call answer `key`).
  - The volume key goes to `keystore`, the store mounts, and from then on it serves every client.
  - `invalid`: a typo, a mounted store, or a RAM disk's key.
  - `key`: the key opens nothing, or the store holds no secrets client.
- **`recoveries() -> u32`:** any client's. On a mounted store, how many records there are; on an unmounted one, `key`. So `recovery` can tell "make a key" from "open with a key".
- **No key is logged.** The service's lines name the PID and counts only (docs/storage 0.27, "The wire").

## KRN (308-KRN-0204)

`init` can mint the client with `mind::blockstore::BADGE_RECOVER` alone. It may hold no other right: `recovery` reads `recoveries` with it, which is allowed to that badge.

## APP (308-APP-0122)

- **The medium's id is not on the wire.** Your `--key-file`'s first line names it. The store logs it only when it is not mounted (`NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS volume-<id>`). If `recovery` needs the id itself, say so: a call that returns it is a small 2.5.
- **`blocks recovery try`** is how the `store` suite checks the refusals. `blocks` never shows a key.
