# 801-KRN-0244 — The TPM lent over pages the firmware lists as reserved: the DEXP's TPM found again

**Type:** kernel (`PLATFORM_MMIO`, `init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S3, S5) · **Roadmap:** stage V · **Constitution:** MC-3.1, MC-11.9, MC-12.1

## Problem

**The DEXP lost its TPM with release 202611140747** (DRV's `KRN/…-DRV-the-dexp-lost-its-tpm-its-device-key-and-wi-fi-with-a-release.md`):
- `init` found node 0 (`mind,tpm2`), and `tpm` said `NO TPM`;
- `keystore` then made a new device key, and the remembered networks did not open under it;
- the same machine and disk, on release 202610302359, find `TPM 2.0 BY INTC, CRB INTERFACE`.

**The change between the two is 100000-KRN-0171.** The TPM's page is the same in both, the TPM2 table's control area page. What is new is `PLATFORM_MMIO`'s refusal of pages that hold RAM: any range of the firmware's memory map other than memory-mapped I/O (UEFI types 11 and 12) counts as RAM. Firmware lists an Intel CRB TPM's registers as reserved (type 0), so the page was refused.
- The refusal was printed only on the kernel's serial line, which the DEXP has none of.
- `init` dropped the error, so nothing on the machine said why.

## Plan

- **RAM means memory the firmware or the system uses** (`nodes::holds_ram`): loader, boot and runtime code and data, conventional, unusable, ACPI reclaim and NVS, persistent and unaccepted memory. Reserved and memory-mapped I/O ranges are not RAM, so a node's registers there are lent.
- **`init` says a node not lent:** `[INIT] tpm: PLATFORM NODE n NOT LENT: <error>`.
- **Unchanged:** the MSI-X table page's check, which guards the kernel's own 2 MiB mapping.

## Acceptance criteria

- **The host test** `registers_listed_as_reserved_or_io_are_not_ram`.
- **The `tls` suite with `swtpm`** (the TPM's page lent and the device key sealed) and the `normal` suite pass on both arches on the maintainer's machines, then in a gate.
- **On the DEXP:** `[TPM] READY` and `DEVICE KEY FROM system/keystore/device.sealed` with a release from `fast-test`. That is DRV's or the maintainer's run (MC-12.1).

## Progress (2026-11-14)

- **In `main` since round 135** (01c3b5aa32), and gated green on the pool. On the maintainer's machines before that:
  - x86 "network, TLS and Effector" (the TPM with `swtpm`) and "boot, display, shell, memory, clock" on x32;
  - aarch64 "network, TLS and Effector", "the board from a device tree, under U-Boot" and "programs, shell and four CPUs" on acc-2.
- **Left: the DEXP's run.** DRV was asked (`DRV/…-KRN-the-dexps-tpm-key-and-radio-the-three-tasks.md`) for the memory map's type at `0xFED40000`, or `[TPM] READY` with a release that has this change.

## Related

[801](801-the-bench-stays-manageable.md), [100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done), [351-KRN-0052](../issues-done/351-KRN-0052-tpm-registers-from-the-firmware.done), [801-NET-0079](../issues-done/801-NET-0079-keystore-keeps-the-slot-befores-keys-and-networks.done) (STO's: `keystore` keeps a sealed key while no TPM answers).
