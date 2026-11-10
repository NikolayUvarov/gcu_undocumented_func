# 351-UPD-0034 — `apply` writes a damaged staged slot again from the store

**Type:** `updater` · **Owner:** `STO`, by agreement with the update track's owner (the kernel session, 2026-11-10) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-6.4, MC-9.3

Numbered from STO's offer of 2026-11-09 (`KRN/2026-11-09T1925Z-STO-two-updater-follow-ups-of-351-upd-0033-an-offer.md` on `internal-dev-messages`), the first of 351-UPD-0033's "not done" points. STO makes it.

## Problem

- **Today:** only `rollback` writes a slot again from the store, for the last known good release.
- **An `apply`** whose staged slot is not as its manifest lists it is refused (`digest`). Only `fetch` writes the slot again, from the source.

## Plan

1. **The store first.** When the store keeps the release the slot was staged with (its version above the running one), `apply` writes the slot from there, with the same checks as a fetch.
2. **Then the usual steps:** it checks the slot and boots it on trial.
3. **`update.wit` 1.5** (minor) says so in `apply`'s text.

## Acceptance criteria

- **The `updatestore` suite** (x86, aarch64):
  - release 6 is staged;
  - a boot file of slot B is damaged on the disk with the machine off;
  - `update apply` boots release 6.
- `docs/update/updater.md` states it.

## Related

[351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done), [351-UPD-0035](351-UPD-0035-the-releases-the-store-keeps-through-update-wit.md).
