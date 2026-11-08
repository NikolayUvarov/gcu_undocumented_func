# 351-KRN-0218 — The updater's own client of the block store

**Type:** `init` · **Owner:** kernel session · **Priority:** P3 · **Status:** open · **Blocked by:** STO's `mind::blockstore::OWNER_UPDATER`, part of [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md) on `claude/relaxed-meitner-5bmhpz` · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.3, MC-9.3

Numbered by the kernel track on 2026-11-08 from STO's request (recorded in `requests-KRN.md` the same day). The plan is STO's, taken as it is.

## Problem

- **MC-9.3** asks that the recovery images and the objects they need be kept from ordinary cleanup. [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md) keeps the running and the last-known-good release in the block store, pinned by the updater, so that collection never frees them and a damaged slot can be written again from them.
- **The updater holds no client of the store.** `init` gives store clients to the shell and to `storefs` alone, both badged `BADGE_GET | BADGE_PUT | BADGE_PUBLISH` (7). Those two share one owner, since the store takes a client's badge as its owner.

## Plan

1. **`init`** gives the updater, when `blockstore` runs, a client of the store badged `mind::blockstore::OWNER_UPDATER | BADGE_GET | BADGE_PUT | BADGE_PUBLISH` (`0x17`), in a slot of its own. `OWNER_UPDATER` (`0x10`) comes with STO's part of 351-STO-0006, on `claude/relaxed-meitner-5bmhpz`. Its pins and its account are then its own: the shell's client cannot end them.
2. **The order:** the updater starts after `blockstore`, so that its first request finds the store mounted. Without the store, it runs as now.
3. **Docs:** `docs/profile/bootstrap.md`'s row for the updater names the client.

## Acceptance criteria

- **Builds** x86 and aarch64. `stat caps` of the updater shows the client, and the store's log names its badge at its first request.
- **The updater's own checks** (UPD's part, `requests-UPD.md`) pass in the `update` suite once that part lands.

## Related

[351-UPD-0033](351-UPD-0033-the-updater-keeps-its-releases-in-the-store.md) (the updater's use of it), [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md).
