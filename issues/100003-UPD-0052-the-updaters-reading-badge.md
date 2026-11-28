# 100003-UPD-0052 — The updater's reading badge: `status`, `metadata` and `kept` alone (`update.wit` 2.1)

**Type:** `updater`, IDL (`update.wit` 2.1) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-28) · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), for [100003-APP-0151](100003-APP-0151-improve-on-mind-core.md) · **With:** [100003-KRN-0291](100003-KRN-0291-a-reading-client-of-the-updater-for-programs.md) · **Constitution:** MC-3.11, MC-2.4, MC-12.4

## Problem

**APP's request of 2026-11-21**, recorded in `requests-UPD.md`.
- `improve` reads the release the machine runs, so that a request a release carries waits for the user's check once the machine runs it.
- It needs a client of the updater that may read and do nothing else.
- Before this task the updater told two clients apart: the Effector gateway's (`BADGE_UPDATE_GATEWAY`) and every other, which could do all but hand over a bundle.

## Plan

- `mind::process::BADGE_UPDATE_READ` = 2.
- `updater/src/rights.rs` decides each call by its badge:
  - **`status`, `metadata`, `kept`:** every client.
  - **`check`:** the shell's (no badge) and the gateway's.
  - **`fetch`, `apply`, `rollback`:** the shell's; the gateway's while init's `remote-update` counts.
  - **`install-…`:** the gateway's alone, while `remote-update` counts.
  - **Every other badge, the reading one among them:** `denied`, logged as `[UPDATER] <CALL> REFUSED (BADGE b, PID p): THIS CLIENT MAY ONLY READ THE UPDATER'S STATE (MC-3.11)`.
  - Before this task any badge but the gateway's was treated as the shell's. Now a badge init does not mint may only read.
- `update.wit` 2.1 says which calls the reading badge may make. No call changes, so it is a minor version.

## Acceptance criteria

1. `updater/src/rights.rs`'s rule, tested on the host (`tests/updater_host.rs`, `the_reading_badge_reads_alone_and_the_gateway_changes_while_remote_update_counts`). **Met on the branch** (22 tests pass).
2. `update.wit` 2.1, its bindings regenerated, `docs/idl/README.md`'s row. **Met on the branch.**
3. On a running system, a program holding the lent client reads `status` and is `denied` `fetch`. This waits for the shell's lending (APP's part of 100003-APP-0151); it is that check's to show.

## Related

[100003-KRN-0291](100003-KRN-0291-a-reading-client-of-the-updater-for-programs.md), 100003-APP-0151, 751-KRN-0185 (the gateway's badge), 751-UPD-0032 (the bundle).
