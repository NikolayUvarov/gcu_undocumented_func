# 351-ASR-0005 — Power loss at every step of an update

**Type:** assurance (fault injection) · **Owner:** `ASR` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** [351-UPD-0006](351-UPD-0006-slots-and-boot-records.md), [351-KRN-0014](351-KRN-0014-trial-boot-and-confirmation.md); later [351-UPD-0007](351-UPD-0007-updater-service.md) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.3, MC-12.2

Numbered by the kernel session at the maintainer's request (2026-10-08); the track is open.

## Problem

MC-9.3 asks that a failure at any point leave a recoverable configuration. Only an injected failure at every point shows that. A test of the happy path does not.

## Plan

- **A QEMU harness that kills the machine at chosen points**, with the disk image kept between runs, not in snapshot mode. The points:
  - each blob write;
  - between the last blob and the boot record;
  - inside the boot record's sector write (a torn write, simulated with a partial sector);
  - during the trial before confirmation;
  - after confirmation.
- **After each kill the machine boots again.** The test checks that it runs a slot whose manifest verifies, and that this slot is the expected one: the old slot before the record is written, and the new one after the confirmation.
- **It runs as a CI group.** The points are listed in `docs/assurance/`.

## Acceptance criteria

Every listed point leaves a machine that boots to the shell on a verified slot, on x86 and aarch64. The table of points and outcomes is in `docs/assurance/`. A test is evidence, not a proof (MC-12.2).

## Related

[351](351-self-update.md).
