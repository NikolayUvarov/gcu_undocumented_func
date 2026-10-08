# 351-KRN-0014 — A trial boot: the flag, the confirmation, the deadline and the updater's grants

**Type:** kernel (ABI) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done) (the records: done), [211-KRN-0012](211-KRN-0012-boot-volume-identity.md) (the `BootInfo` change it rides on) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.3, MC-3.11, MC-11.1

## Problem

A slot booted on trial must be confirmed, or else replaced at the next boot. Today:

- the system cannot tell whether it runs on trial;
- nothing marks a boot as good;
- a hang never restarts the machine, because there is no watchdog (`docs/profile/threat-model.md`: driver hangs are not detected).

## Plan

- **`BootInfo`** carries the slot, the trial flag and the slot's manifest hash from the bootloader. It is part of the same ABI version as 211-KRN-0012's field. The launch record of 350-UPD-0004 reads them.
- **Confirmation.**
  - `init` confirms after `[INIT] READY` and its health check: every boot service started, and `vfs_server` mounted the boot volume.
  - It asks `updater`, or `vfs_server`'s update zone if `updater` is not running, to write the confirmed record.
- **A deadline.**
  - On a trial boot the kernel restarts the machine if no confirmation came within a time the bootloader passes (default 120 s).
  - The confirmation is a system call only `init` may make (platform privilege).
  - A hang before `init` runs is covered too, because the kernel's own tick enforces the deadline.
- **Grants.** New request flags let `init` give `updater` exactly its authorities:
  - TLS;
  - the update zone;
  - a network grant;
  - restart through `init`.

  These are an ABI change in `common/abi.rs` and `libmind::process`, with the version bump of 211-KRN-0012.
- **Tests:** the trial flag reaches `init`; a confirmed trial stays; a trial that never confirms restarts at the deadline in QEMU.

## Acceptance criteria

In QEMU:

- an unconfirmed trial restarts by itself at the deadline, and the next boot falls back (with 351-UPD-0006);
- a confirmed one does not restart;
- only `init` can confirm.

## Related

[351](351-self-update.md), [350](350-signed-boot-images.md).
