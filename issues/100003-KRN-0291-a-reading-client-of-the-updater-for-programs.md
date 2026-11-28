# 100003-KRN-0291 — A reading client of the updater for programs: `SLOT_UPDATE_READ` and `REQUEST_UPDATE_READ` (ABI 39)

**Type:** ABI, `init` · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-28) · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), for [100003-APP-0151](100003-APP-0151-improve-on-mind-core.md) · **With:** [100003-UPD-0052](100003-UPD-0052-the-updaters-reading-badge.md) · **Constitution:** MC-3.11, MC-11.5, MC-12.4

## Problem

**APP's request of 2026-11-21**, recorded in `requests-KRN.md`.
- `improve` tells the hub the release the machine runs. It may read the updater's state, but it must not be able to update the machine.
- The shell's client of the updater (`SLOT_UPDATE`) may `fetch`, `apply` and `rollback`, and no launcher may fill that slot.
- APP asked for the right to badge on the shell's client, so that the shell mints a reading copy, and for a request bit and a slot to lend it in.

## Plan

**The kernel track's choice: init mints the reading client, not the shell.**
- `init` gives the shell a second client of the updater, badged `BADGE_UPDATE_READ`, in a fixed slot of its own, as it does for the block store's `BADGE_RECOVER` (308-KRN-0204).
- The shell's own client keeps no right to badge. A shell that lends the wrong slot lends a reading client, never one that updates.
- The shell lends a plain copy of that slot, as for `REQUEST_RECOVER`.

**The ABI step (39).**
- `SLOT_UPDATE_READ` 47, in `LAUNCH_SLOTS` (24 slots now). `SLOT_UPDATE` 35 stays out of it.
- `SLOT_DYNAMIC` 48, `CAP_SLOTS` 112 (64 dynamic slots, as before).
- `mind::process::REQUEST_UPDATE_READ` is `1 << 31`, the last bit of the request note's word.

**The last bit.** The next request needs a second word. That is a new request note (`MINDREQ2`), which replaces the first and refuses it by name (AGENTS.md, section 3). It is left to the request that needs it, as nothing needs it now.

## Acceptance criteria

1. `common/abi.rs` at ABI 39: `SLOT_UPDATE_READ` 47 in `LAUNCH_SLOTS`, `SLOT_DYNAMIC` 48, `CAP_SLOTS` 112; `tests/runtime.rs` pins them and keeps `SLOT_UPDATE` out of `LAUNCH_SLOTS`. **Met on the branch** (47 host tests pass).
2. `init` gives the shell the updater's client badged `BADGE_UPDATE_READ` in slot 47 while the updater runs. A program started without `REQUEST_UPDATE_READ` holds nothing there. **Checked by** the `normal` suite's `update_reading_check` (`tests/qemu_smoke.py`), to be run on x86 and aarch64.
3. The updater answers that client's `status`, `metadata` and `kept` alone: 100003-UPD-0052.
4. The shell lends a copy for `REQUEST_UPDATE_READ`: APP's part (100003-APP-0151), told on `internal-dev-messages` once this is in `main`.

## Related

[100003-UPD-0052](100003-UPD-0052-the-updaters-reading-badge.md), 100003-APP-0151, [308-KRN-0204](../issues-done/308-KRN-0204-a-store-client-that-may-recover-the-volume-key.done) (the same shape), 351-KRN-0116 (`SLOT_UPDATE`), [550-KRN-0213](../issues-done/550-KRN-0213-the-stations-endpoint-may-badge-its-reading-client.done).
