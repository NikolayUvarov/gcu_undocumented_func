# 351-APP-0132 — `update kept`: the releases the block store keeps for the updater

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** — (`update.wit` 1.5 `kept` is in `main` since round 116) · **Main task:** [351](351-self-update.md) · **Roadmap:** track G · **Constitution:** MC-9.2, MC-12.3

**Asked by:** STO, for [351-UPD-0035](351-UPD-0035-the-releases-the-store-keeps-through-update-wit.md), in [requests-APP.md](requests-APP.md), 2026-11-10. Numbered on 2026-11-10.

## Problem

Which releases the block store keeps for the updater, so which ones a rollback or a damaged slot can come back from (351-UPD-0033, 0034), shows only in the updater's log. `update.wit` 1.5 gives them to any client:
- `kept: func() -> list<kept-release, 8>`, newest first, none without the store or on a RAM disk;
- each `kept-release` is the version, the pin that keeps it, the tree's root (a CID in binary, 36 bytes) and the bytes of the files its manifest lists for a slot, each counted once (0: its blobs were not read).

## Plan

- **`update kept`** in the shell (`shell/src/updates.rs` for the words and lines, `updatectl.rs` for the call): one line a release, `UPDATE: KEPT VERSION 6, PIN 2, 26 677 432 BYTES, ROOT bafy…`, the root in `mind::cid`'s text form; with none, `UPDATE: NO RELEASE KEPT IN THE BLOCK STORE`.
- **`update`'s usage** and `help` name it; `docs/tools` (English and Russian) too.

## Acceptance criteria

- **Host tests:** the word, the lines (the bytes grouped by thousands, a release whose bytes were not read, none kept, a root that is not a CID).
- **QEMU:** the `updatestore` suite reads the releases through the shell after a fetch, versions 6 and 5 with the pins and roots the updater logged (351-UPD-0035's second criterion); where the updater keeps none (the stub), `update kept` says so.

## Related

[351-UPD-0035](351-UPD-0035-the-releases-the-store-keeps-through-update-wit.md), [351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done), [351-APP-0029](../issues-done/351-APP-0029-update-in-the-shell.done).
