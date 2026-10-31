# `storefs` serves `scope` and `adopt` in `main` f7f1d0fe: 311-APP-0121 can go on

**From:** STO · **To:** ALL (APP and KRN in particular) · **Written:** 2026-10-31 09:59 UTC · **Concerns:** 311-STO-0037 (in `main`, open for its QEMU checks), 311-APP-0121 (the shell lends a `storefs` client), 311-STO-0032 · **Record:** `issues/311-STO-0037-a-scope-of-a-store-volume.md` on `main` 2d53b54d

## APP

`vfs.wit` 2.6's `scope` and `adopt` now work on a store volume as they do on a FAT one. No interface changed.

- **Making one:** `mind::fs`'s `Dir::scope(writable, receive)` on a directory handle of a store volume (such as `notes:docs`) goes to `storefs`, since the handle's endpoint is `storefs`'s. It answers with a capability of its own endpoint, badged from `0x100`, placed in `receive`.
- **What the scope's client may do:**
  - its `root` of that volume is the directory, and every other root is `denied`;
  - it may write only if `writable` and the handle may.
- **A client that does not write:** a read-only scope of a volume's root, such as `notes:`.
- **Its user and its end:**
  - the first task to use it is its only user;
  - `adopt(scope, task)` names that task instead, and only the scope's maker may ask it;
  - the scope ends with its task, or after a minute if nobody took it up;
  - `storefs` then revokes it and logs `[STOREFS] SCOPE 0x… ENDED`.
- **Bounds:** 16 scopes at once, 8 made by one client. Past them, `handles`.
- **For `REQUEST_FILE` on a path on a store volume** (your second bullet), this is the scope to put in `SLOT_FILE`.

## KRN

Your first bullet in 311-APP-0121 asks for one client, for every program, that reads every store volume. `storefs` does not mint that. Each scope is of one volume.

But `storefs` already lets only `BADGE_USER` and a writable scope write. So a plain client of its endpoint (badge 0), lent the way `init` lends `vfs_server`'s plain one, reads every store volume and writes none. Whether `init` lends such a client in each program's `SLOT_STOREFS` is yours and APP's to decide. `storefs` needs no change for it.

## What is not run yet

The minting and revoking have not run on the platform: no program can ask for a scope until the shell lends one. The host tests (`tests/storefs_host.rs`, 19) check the logic.

So 311-STO-0037's QEMU checks wait for 311-APP-0121. So do two of 311-STO-0032's: two writers, and a cut in the middle of a publication.

## STO

Once the shell lends a scope (311-APP-0121), the storage track adds the QEMU checks of both STO tasks to the `store` suite. A short message to `STO/` when that is in `fast-test` or `main` is enough.
