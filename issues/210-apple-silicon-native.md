# 210 — Apple Silicon Macs natively (M1 first)

**Type:** porting (main task) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** a Mac with M1 to test on (a person: [issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)); [205](205-aarch64-boards.md) (boards with UEFI first) · **Roadmap:** track H · **Constitution:** MC-1.5, MC-9.1, MC-12.1

Requested by the user (2026-10-06), recorded by the tools track in the porting track's request file. Moved from `PRT` to the Apple Silicon track `APL` on 2026-10-08, at the maintainer's request ([TRACKS.md](../TRACKS.md)); the number stays. Its tasks are `210-APL-MMMM` (below). Running in a virtual machine on a Mac is main task [600](600-apple-silicon-mac-vm-host.md); the guide is [docs/apple-silicon.md](../docs/apple-silicon.md).

## Problem

Asked by the user (2026-10-06): MIND Core should run natively on a Mac with Apple Silicon (M1, M2, M3), not only in a virtual machine. Such a Mac has no UEFI and no ACPI: iBoot boots it, and a device tree describes it. Its interrupt controller is Apple's AIC (AIC2 on the M1 Pro, Max and Ultra and on M2 and later), not a GIC, and the timer interrupt arrives as an FIQ. Its CPUs start with a spin table, not PSCI. Every DMA-capable device sits behind its own IOMMU (DART). The internal keyboard and trackpad (SPI or MTP), the NVMe (ANS with RTKit firmware), Wi-Fi and sound are Apple's own; the Asahi Linux project documented them over several years.

Today the user can run the aarch64 build in a virtual machine on such a Mac: `03_run_qemu_aarch64.sh` uses the hypervisor (HVF) there (tools track, not yet tested on a Mac; main task [600](600-apple-silicon-mac-vm-host.md)).

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

## Tasks

| Task | Step |
|---|---|
| [210-APL-0001](210-APL-0001-boot-through-m1n1-and-u-boot.md) | 1: boot through m1n1 and U-Boot, `BOOTAA64.EFI` from U-Boot's UEFI, entry at EL2 |
| [210-APL-0002](210-APL-0002-board-from-the-device-tree.md) | 2: the board from the device tree where there is no ACPI |
| [210-APL-0003](210-APL-0003-aic-and-the-timer-fiq.md) | 2: the AIC (AIC2) and the timer's FIQ |
| [210-APL-0004](210-APL-0004-cpus-through-the-spin-table.md) | 2: the other CPUs through the spin table |
| [210-APL-0005](210-APL-0005-reset-without-psci.md) | reset through the watchdog, as there is no PSCI |
| [210-APL-0006](210-APL-0006-samsung-style-uart-console.md) | 3: the console on the Samsung-style UART |
| [210-APL-0007](210-APL-0007-dart-dma-boundary.md) | 2, 3: the DARTs as the DMA boundary; 16 KiB pages |
| [210-APL-0008](210-APL-0008-usb-on-type-c-ports.md) | 3: USB on the Type-C ports (DWC3, ATC PHY, PMGR) |
| [210-APL-0013](210-APL-0013-own-stage-two-instead-of-u-boot.md) | 1, later: our own second stage after m1n1, without U-Boot |
| [210-APL-0014](210-APL-0014-own-first-stage-instead-of-m1n1.md) | 1, last: our own first stage started by iBoot, without m1n1 |

Step 4 stays here until work on it starts.

## Acceptance criteria

A Mac mini or MacBook with M1 boots to the shell from a USB stick with an external USB keyboard. The aarch64 profile gets an `aarch64/apple-m1-0` entry with its TCB (m1n1 and U-Boot included) and what was tested on which machine.

## Related

[201](../issues-done/201-aarch64-boot.done), [205](205-aarch64-boards.md), [206](../issues-done/206-pin-controllers-from-firmware.done), [600](600-apple-silicon-mac-vm-host.md), [docs/apple-silicon.md](../docs/apple-silicon.md).
