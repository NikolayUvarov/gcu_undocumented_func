# 351-UPD-0007 — The `updater` service

**Type:** update (service) · **Owner:** `UPD` track · **Priority:** P1 · **Status:** open · **Blocked by:** [351-UPD-0005](351-UPD-0005-release-and-publish.md), [351-UPD-0006](351-UPD-0006-slots-and-boot-records.md), [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md), [351-KRN-0014](351-KRN-0014-trial-boot-and-confirmation.md), the HTTPS download from `NET` (`requests-NET.md`) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-8.5, MC-9.2–9.4, MC-3.11

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

## Acceptance criteria

In QEMU against the test server of 351-UPD-0005:

- `update check`, `fetch` and `apply` move a device from release N to N+1, which then confirms;
- a changed blob, a bad signature, an older version and expired metadata are each refused, with nothing written to the boot records;
- a download cut midway resumes.

## Related

[351](351-self-update.md).
