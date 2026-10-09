# 210-APL-0002 — The board from the device tree where there is no ACPI

**Type:** porting (kernel) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** — ([210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done): the device tree's address in `BootInfo.device_tree`, ABI 4, done) · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

Part of main task [210](210-apple-silicon-native.md), plan step 2.

## Problem

The aarch64 kernel reads the board from ACPI (`arch/aarch64/acpi.rs` into `board.rs`): the GIC from the MADT, the console from the SPCR, the timer's interrupt from the GTDT, the ECAM from the MCFG, PSCI's conduit from the FADT. On a Mac, U-Boot gives a flattened device tree (FDT) instead (expected). The bootloader does not pass it on: `BootInfo` has `acpi_rsdp` and no field for a device tree.

## Plan

- `BootInfo` gains the device tree's address: done in [210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done) (`BootInfo.device_tree`, ABI 4). The bootloader takes it from the UEFI configuration table, and the kernel checks its header (`firmware::device_tree`). The `devicetree` suite boots QEMU `virt,acpi=off` and reads the kernel's line from the screen.
- A bounds-checked FDT reader in the kernel, as `acpi.rs` is for ACPI. Memory still comes from the UEFI memory map. From the tree: the interrupt controller (`apple,aic`, `apple,aic2`), the timer's interrupts, the CPUs (`reg` as the MPIDR, `enable-method`, `cpu-release-addr`), the UART (`apple,s5l-uart`) and `/chosen` `stdout-path`, the DARTs, the USB controllers, the watchdog and PCIe. `board.rs` keeps one record, whichever source filled it.
- Host tests with device trees compiled from Linux's `arch/arm64/boot/dts/apple/` for t8103 (the M1 Mac mini and MacBook Air), whose files are marked `GPL-2.0+ OR MIT` and can be used under MIT (to be checked, and recorded in THIRD_PARTY.md); and with truncated and random blobs, which must not panic.
- A test of the device tree path without a Mac: QEMU `virt` with `acpi=off`, where EDK2 is expected to hand over a device tree instead of ACPI (GIC, PL011, PSCI there).
- The kernel's code is `PRT`'s directory: done with `PRT`.

## Acceptance criteria

Host tests read the devices above from the t8103 trees and refuse malformed blobs without a panic. On QEMU `virt,acpi=off` the kernel takes the board from the device tree and boots to the shell, checked by a suite. On an M1 the kernel reports the board it read, once 210-APL-0006 gives it a console.

## Related

[210](210-apple-silicon-native.md), [210-APL-0001](210-APL-0001-boot-through-m1n1-and-u-boot.md), [205](205-aarch64-boards.md) (the board from ACPI), [206](../issues-done/206-pin-controllers-from-firmware.done).
