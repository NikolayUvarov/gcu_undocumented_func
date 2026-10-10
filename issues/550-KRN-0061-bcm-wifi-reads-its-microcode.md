# 550-KRN-0061 — `bcm_wifi` gets a read-only VFS client for its microcode

**Type:** kernel (`init`) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** tracks A and D, for main task [550](550-network-on-real-hardware.md) · **Constitution:** MC-1.5 (a driver gets only what it needs), MC-11.1 (authority only through held capabilities)

## Problem

This is the drivers track's request in `requests-KRN.md` (2026-10-10), for [550-DRV-0023](550-DRV-0023-bcm4331-microcode-runs.md), stage 2 of the MacBook Pro's Wi-Fi.

- Stage 2 of `bcm_wifi` loads Broadcom's microcode into the BCM4331's 802.11 core.
- The maintainer's build copies the file onto the written disk under `data/firmware/b43/` (`scripts/proprietary.sh`), never into the image.
- `init` gave `bcm_wifi` BAR0 only ([550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done)), so the driver could not read the file. Stage 2 then logs that it is skipped, and the system runs on.

## Plan

- `init` lends `bcm_wifi` a client of `vfs_server` in `SLOT_VFS`, as it does `gpio` for `hwdocs/`.
- The client is unbadged, and `vfs_server` lets an unbadged client only read. Only `BADGE_USER` writes, and only the badge of a private directory under `system/` opens it.
- The client is lent only when `vfs_server` runs, so stage 1 still starts without it.
- `svc` lists it among what `bcm_wifi` holds.
- **Note (the storage session, 2026-10-10):** when `main` reached its branch, the client became badged `BADGE_READER`, as every other reader's client of `vfs_server` is since [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done). An unbadged client can still be badged by whoever holds it, so the third criterion below needs the reader's badge.

## Acceptance criteria

1. **On the MacBook Pro:**
   - `bcm_wifi` opens `data/firmware/b43/ucode29_mimo.fw` and logs its size;
   - without the file it logs that the file is missing and runs on;
   - it cannot write anywhere or open a private directory.
2. **On QEMU** (no such chip): `bcm_wifi NOT STARTED: NO DEVICE` as before. The gate's suites pass.

## Related

[550-DRV-0023](550-DRV-0023-bcm4331-microcode-runs.md), [550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done).
