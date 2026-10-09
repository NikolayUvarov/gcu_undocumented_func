# 211-DRV-0002 — `ahci`: every port with a disk and every controller

**Type:** driver · **Owner:** `DRV` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done) (which disk is the boot disk) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.1

Numbered by the kernel session at the maintainer's request (2026-10-08); the `DRV` track is open.

## Problem

`init` starts `ahci` for the first controller of class 01:06:01 only (`init/src/main.rs:192-197`), and `ahci` takes the first port with a link and an ATA signature (`ahci/src/main.rs:34`). On a PC with the 860 PRO on another port, or with two SATA disks, the system may use the wrong disk or none.

## Plan

- `ahci` serves every port with a disk, each as its own block client with the port in its name. `init` starts one instance per AHCI controller.
- The boot disk is the one whose volume matches `BootInfo`'s identity (211-KRN-0012).
- Out of scope, each its own issue later: NCQ, TRIM (DATA SET MANAGEMENT), hot plug, RST/VMD.
- A test: QEMU with an AHCI controller holding two disks, booting from the second port.

## Acceptance criteria

With two SATA disks, the system boots from and writes to the disk it booted from, whichever port it is on. The `ahci` and `block` suites pass.

## Related

[211](211-intel-pc-from-a-sata-ssd.md), [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done).
