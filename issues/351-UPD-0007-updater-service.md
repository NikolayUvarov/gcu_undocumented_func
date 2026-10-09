# 351-UPD-0007 — The `updater` service

**Type:** update (service) · **Owner:** `UPD` track · **Priority:** P1 · **Status:** open · **Blocked by:** [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done), [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done), [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md), [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done), HTTPS downloads from `NET` ([351-NET-0002](351-NET-0002-https-for-programs.md); the download with resume itself is [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done)) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-8.5, MC-9.2–9.4, MC-3.11

Numbered by the kernel session at the maintainer's request (2026-10-08), before the track had an owner.

## Problem

Something on the device has to turn a published release into a staged, verified slot, and only that.

## Plan

- **A service started by `init`** with only what it needs:
  - the update zone of `vfs_server`;
  - HTTPS through `tls`, later SSH;
  - a network grant for the update server;
  - the right to ask for a restart.

  It cannot write anything else (MC-3.11: the manifest it reads grants nothing).
- **Interface `idl/update.wit`:** `check`, `fetch`, `apply`, `status`, `rollback`.
  - `check` reads the channel, verifies its signature and expiry, and compares versions with the running one and the minimum.
  - `fetch` downloads every blob the manifest names into the inactive slot, resuming a partial file, and checks every hash.
  - `apply` writes the trial boot record and asks `init` to restart.
  - `rollback` writes a record that points at the last confirmed slot.
- **Sources:** an HTTPS URL, an `ssh://` URL in phase 3, or a directory on a disk (for a USB stick). One verification path serves all three.
- **Status** is visible to the shell and `sysinfo`: current, staged, trial and last-known-good versions, and the last error.
- **Images with slots** (moved here from 351-UPD-0006, which left the images in the root layout until a slot can be staged):
  - `scripts/make_usb_image.py` puts the build in slot A, confirmed, with an empty slot B, as `scripts/boot_slots.py layout` does;
  - the programs and docs that name `kernel.elf` at the root (`files`, the disk-writing guides) follow.
- **The slot's manifest** is the release's own. The bootloader checks the slot's kernel and services against it by name ([slots.md](../docs/update/slots.md)), so the updater writes the release's `MANIFEST` and `MANIFEST.SIG` into the slot as published and never signs on the device.

## Acceptance criteria

In QEMU against the test server of 351-UPD-0005:

- `update check`, `fetch` and `apply` move a device from release N to N+1, which then confirms;
- a changed blob, a bad signature, an older version and expired metadata are each refused, with nothing written to the boot records;
- a download cut midway resumes.

## Related

[351](351-self-update.md).
