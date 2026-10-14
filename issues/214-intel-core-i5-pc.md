# 214 — An Intel Core i5 PC: the maintainer's i5-4430 (Haswell) with a GeForce GT 1030

**Type:** main task, porting · **Owner:** `PRT` (kernel session), with `KRN` and `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.3

## Problem

The maintainer's runs on 2026-10-13 used `fast-test` 28d9c3d and the same USB stick (7825:A2A4) each time:
- **A first try,** known only from a photo of the screen. The stick was not set up on xHCI port 8, so there was no boot volume, no program and no log.
- **Boot 1** (`log:boot0001.log`, `log:hw0001.txt`): the stick on port 8 again, at high speed, and this time it was set up.
- **Boot 2** (`log:boot0002.log`, `log:hw0002.txt`): the stick in a blue port, on port 16 at SuperSpeed; `wm` with its windows.

Both boots also wrote 17 ACPI tables to `log:acpi/`. They were read outside the repository.

**The machine:**
- **Processor:** an Intel Core i5-4430 (Haswell: family 6, model 0x3C, stepping 3, microcode 0x12), four cores without Hyper-Threading (APIC IDs 0, 2, 4 and 6).
- **Memory:** about 16 GiB (a frame pool of 15744 MiB).
- **Board:** MSI MS-7817 (subsystem 1462:7817), with an H81 PCH (LPC 8086:8C5C) and AMI firmware.
- **Firmware tables:** VT-d (a DMAR with two remapping units), no TPM.
- **Screen:** a GeForce GT 1030 (10DE:1D01) drives it through its GOP at 1024 × 768. The processor's graphics (8086:0412) are there too.
- **Disk:** a 240 GB SATA disk on AHCI port 0 (468862128 sectors).

**What worked:**
- **The start:** the bootloader, the signed manifest and the kernel, with 4 CPUs, xAPIC, SMEP and the LAPIC tick measured on the ACPI PM timer. The kernel moved the PCH's USB ports from EHCI to the xHCI ("USB 2 FFF OF MASK FFF, USB 3 3 OF MASK 3").
- **The services:** every boot service started, and so did the shell. On boot 2 `wm` ran with its windows.
- **USB on the xHCI 8086:8C31:**
  - the stick at high speed on boot 1 and at SuperSpeed on boot 2;
  - the low-speed keyboard 046D:C31C typed;
  - the reports of the low-speed mouse 046D:C01D (report protocol, 12-bit X and Y) reached `usb_hid`.
- **Disks:** AHCI (8086:8C02) found the SATA disk. Each boot wrote its log, its hardware report and the ACPI tables to the log partition.

**What did not:**
- **The stick on port 8 (USB 2)** failed ADDRESS DEVICE on the first try, though boot 1 set it up on the same port (requests-DRV.md).
- **Sound:**
  - `audio_gw` took the processor's display audio (00:03.0, 8086:0C0C), and its codec did not answer ("CORBRP 1 RIRBWP 0"). The screen is on the GT 1030, so the processor's display, which that codec depends on, is likely off.
  - It did not try the analog controller (00:1B.0, 8086:8C20) (requests-DRV.md).
- **Ethernet:** the Realtek RTL8168 (10EC:8168, revision 0C, 03:00.0) has no driver (requests-DRV.md).
- **The screen at 1024 × 768:** this is the firmware's mode, which the bootloader keeps. Choosing the monitor's own mode needs its EDID ([177](177-display-output.md), step 0).
- **The GT 1030 and the processor's graphics** have no driver: the GOP's framebuffer is all there is ([174](174-full-use-of-pc-hardware.md), [177](177-display-output.md)).

## Plan

1. **The drivers track's part,** asked in [requests-DRV.md](requests-DRV.md):
   - the stick on a USB 2 port;
   - the analog HDA codec;
   - the RTL8168 Ethernet card, the same one as on the Ryzen machine ([213](213-amd-ryzen-pc.md)).
2. **The screen's mode,** with 177's step 0.
3. **VT-d:** this machine's two DMAR units make it a target for 174's IOMMU step on hardware.
4. **Evidence:** each run's evidence is for this machine and this build only (MC-12.1).

## Acceptance criteria

- The stick is set up on each of this machine's ports.
- Sound plays through its analog output.
- Its Ethernet card gets an address by DHCP.
- Each finding above is a closed task or a numbered one.

## Related

[211](211-intel-pc-from-a-sata-ssd.md) (the run from the 860 PRO, still waiting), [213](213-amd-ryzen-pc.md) (the Ryzen machine, the same build), [174](174-full-use-of-pc-hardware.md), [177](177-display-output.md), [requests-DRV.md](requests-DRV.md).
