# 210-APL-0013 — Our own second stage after m1n1, instead of U-Boot

**Type:** porting (boot) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0001](210-APL-0001-boot-through-m1n1-and-u-boot.md) (the first boot through U-Boot shows the machine works), a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-9.1, MC-12.1

Asked by the maintainer (2026-10-08): why not write our own boot loader for Apple Silicon. This task and [210-APL-0014](210-APL-0014-own-first-stage-instead-of-m1n1.md) are the two ways to do it.

## Problem

The plan of [210-APL-0001](210-APL-0001-boot-through-m1n1-and-u-boot.md) boots through three foreign stages: iBoot, m1n1 (MIT) and U-Boot (GPL-2.0). U-Boot is there only to give `BOOTAA64.EFI` a UEFI environment. It is large, it sits in the trusted computing base (MC-9.1), and its licence keeps it outside the repository.

Per Asahi Linux's documentation (not checked here), m1n1 can start a payload appended to it directly: a kernel image with a device tree that m1n1 builds from Apple's device tree (ADT). It also hands over the framebuffer that iBoot set up.

## Plan

- **A second stage of our own, `mindboot-m1n1`, started by m1n1 as its payload in place of U-Boot.** It is a small aarch64 program, under `bootloader/` or beside it, decided with `PRT`.
  - It reads the device tree and the framebuffer m1n1 passes. The kernel's device-tree reading is [210-APL-0002](210-APL-0002-board-from-the-device-tree.md); the loader reuses that parser.
  - It finds the kernel and the boot services. Either they are appended to the payload, or the loader reads them from the EFI system partition. The second needs an NVMe driver for Apple's ANS in the loader; the first does not, so it comes first.
  - It fills `BootInfo` as the UEFI loader does: memory map, framebuffer, the board's description instead of the ACPI pointer. Changing `BootInfo` is a `KRN` task (ABI).
- **Install.** The guide gains the steps to put the payload where m1n1 looks for it, per Asahi's documentation. U-Boot is then not needed.
- **TCB.** The boot chain becomes iBoot, m1n1 (MIT) and our own code. If the repository ever ships m1n1 or parts of it, THIRD_PARTY.md records them; MIT allows it.

## Acceptance criteria

On an M1 Mac, m1n1 starts `mindboot-m1n1`, which starts the kernel without U-Boot. The kernel reaches the same point as in 210-APL-0001's criteria. The guide (both languages) has the steps, and the future profile's TCB lists the chain.

## Related

[210](210-apple-silicon-native.md), [210-APL-0001](210-APL-0001-boot-through-m1n1-and-u-boot.md), [210-APL-0002](210-APL-0002-board-from-the-device-tree.md), [210-APL-0014](210-APL-0014-own-first-stage-instead-of-m1n1.md).
