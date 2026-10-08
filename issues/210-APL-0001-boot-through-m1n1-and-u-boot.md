# 210-APL-0001 — Boot through m1n1 and U-Boot: `BOOTAA64.EFI` from U-Boot's UEFI, entry at EL2

**Type:** porting (boot) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-9.1, MC-12.1

Part of main task [210](210-apple-silicon-native.md), plan step 1.

## Problem

An Apple Silicon Mac boots through Apple's iBoot, which starts no UEFI application. Asahi Linux boots through its own m1n1 (MIT), which sets up the machine and a framebuffer and starts U-Boot (GPL-2.0). U-Boot gives a UEFI environment: boot services, a GOP over m1n1's framebuffer, a memory map and the device tree as a configuration table, but no ACPI (expected, per Asahi's documentation; not checked here).

What MIND Core does today:

- The bootloader passes the kernel the ACPI root pointer from the configuration table, and the aarch64 kernel reads the board from ACPI. Without ACPI it keeps QEMU `virt`'s interrupt controller addresses as defaults, which on a Mac name other hardware.
- The kernel runs at EL1 and has no code for EL2. U-Boot on these Macs is expected to start UEFI applications at EL2, and Apple's cores are expected to fix `HCR_EL2.E2H` at 1.
- The kernel prints only on a serial port, and a Mac has no console the kernel drives yet (210-APL-0006).

## Plan

- Install m1n1 and U-Boot with the Asahi installer (its choice of a UEFI environment only, per its documentation), on a Mac set aside for it. Record the steps in the guide, in both languages.
- Put `EFI/BOOT/BOOTAA64.EFI` and the system on a USB stick, or on the EFI system partition the installer makes, and check that U-Boot starts it.
- Entry at EL2: the bootloader or the kernel sets `HCR_EL2` (with `E2H` as the CPU fixes it) and drops to EL1, or the kernel runs at EL2. Decided with `PRT`, which owns `kernel/src/arch/` and the bootloader's architecture lines.
- Without ACPI and without a device tree it knows, the kernel stops instead of using `virt`'s addresses.
- A sign of life that needs no driver, for example a mark on the GOP framebuffer.
- The boot chain (iBoot, m1n1, U-Boot) in the TCB of the future profile (MC-9.1). m1n1 and U-Boot stay separate firmware stages, outside the image; THIRD_PARTY.md names them if the repository ever ships them.

## Acceptance criteria

On an M1 Mac, U-Boot starts `BOOTAA64.EFI`. The bootloader loads the kernel and hands over the GOP framebuffer; the kernel shows on the screen that it runs, and stops without touching `virt`'s addresses. The steps are in [docs/apple-silicon.md](../docs/apple-silicon.md), in both languages.

## Related

[210](210-apple-silicon-native.md), [210-APL-0002](210-APL-0002-board-from-the-device-tree.md), [201](../issues-done/201-aarch64-boot.done), [205](205-aarch64-boards.md).
