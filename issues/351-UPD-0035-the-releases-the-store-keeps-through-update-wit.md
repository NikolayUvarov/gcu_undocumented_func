# 351-UPD-0035 — The releases the store keeps, through `update.wit`

**Type:** `updater`, interface · **Owner:** `STO`, by agreement with the update track's owner (the kernel session, 2026-11-10) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-12.3, MC-12.4

Numbered from STO's offer of 2026-11-09 (`KRN/2026-11-09T1925Z-STO-two-updater-follow-ups-of-351-upd-0033-an-offer.md`), the second of 351-UPD-0033's "not done" points. STO makes it.

## Problem

Which releases the store keeps for the updater (the running one and the last known good one, 351-UPD-0033) shows only in the updater's log. No client can ask for them.

## Plan

1. **A call `kept`** (`update.wit` 1.5, minor), for any client of the updater: each kept release's version, pin, root and bytes.
2. **For APP:** a request that the shell's `update` shows them.

## Acceptance criteria

- **A host test** covers the call's encoding.
- **The `updatestore` suite** reads the kept releases through the shell, once APP has the command.

## Related

[351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done), [351-UPD-0034](351-UPD-0034-apply-writes-a-damaged-staged-slot-again-from-the-store.md).
