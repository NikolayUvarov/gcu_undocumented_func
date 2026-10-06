# 210 — Apple Silicon Macs natively (M1 first)

**Type:** porting (main task) · **Owner:** `PRT` · **Priority:** P3 · **Status:** open · **Blocked by:** a Mac with M1 to test on (a person: [issues-human](../issues-human/README.md)); [205](205-aarch64-boards.md) (boards with UEFI first) · **Roadmap:** track H · **Constitution:** MC-1.5, MC-9.1, MC-12.1

Requested by the user (2026-10-06), recorded by the tools track in the porting track's request file. Its tasks are `210-PRT-MMMM` (TRACKS.md): device tree, AIC, spin table, DART, DWC3 — each its own task when work starts.

## Problem

Asked by the user (2026-10-06): MIND Core should run natively on a Mac with Apple Silicon (M1, M2, M3), not only in a virtual machine. Such a Mac has no UEFI and no ACPI: iBoot boots it, and a device tree describes it. Its interrupt controller is Apple's AIC (AIC2 on M2 and later), not a GIC, and the timer interrupt arrives as an FIQ. Its CPUs start with a spin table, not PSCI. Every DMA-capable device sits behind its own IOMMU (DART). The internal keyboard and trackpad (SPI or MTP), the NVMe (ANS with RTKit firmware), Wi-Fi and sound are Apple's own; the Asahi Linux project documented them over several years.

Today the user can run the aarch64 build in a virtual machine on such a Mac: `03_run_qemu_aarch64.sh` uses the hypervisor (HVF) there (tools track, untested on macOS).

## Plan

1. **Boot as Asahi does:**
   - The user installs Asahi's m1n1 and U-Boot with the Asahi installer. This means lowering the boot security of the macOS volume group once in recoveryOS; macOS stays.
   - U-Boot gives a UEFI environment with a framebuffer (GOP) and the memory map, so `BOOTAA64.EFI` loads unchanged. m1n1 is MIT; U-Boot is GPL-2.0 and stays a separate firmware stage (THIRD_PARTY.md).
2. **Kernel:**
   - the device tree (FDT) where ACPI is absent: the UART, the interrupt controller, the timer, memory, PCIe, the pin controllers;
   - the AIC with FIQ;
   - CPUs through the spin table;
   - 16 KiB pages for the DART if the CPU's 4 KiB granule cannot be used with it.
3. **Drivers, first target:** the shell on the framebuffer with an external USB keyboard and the system on a USB stick.
   - DART: each driver's DMA through its own IOMMU, which gives MIND Core a real DMA boundary (MC-1.5).
   - USB on the Type-C ports: DWC3 in host mode (xHCI, `usb_host`), its PHY (ATC) and power (PMGR).
   - The Samsung-style UART for the console where it is reachable.
4. **Later, as separate issues:** the internal keyboard and trackpad, NVMe through ANS and RTKit, sound, Wi-Fi, the display controller (DCP). Only reimplemented from documentation: Linux's drivers are GPL and cannot be copied into MIND Core (MIT or Apache-2.0).

## Acceptance criteria

A Mac mini or MacBook with M1 boots to the shell from a USB stick with an external USB keyboard. The aarch64 profile gets an `aarch64/apple-m1-0` entry with its TCB (m1n1 and U-Boot included) and what was tested on which machine.

## Related

[201](../issues-done/201-aarch64-boot.done), [205](205-aarch64-boards.md), [206](../issues-done/206-pin-controllers-from-firmware.done).
