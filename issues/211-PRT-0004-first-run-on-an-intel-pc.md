# 211-PRT-0004 — The first run on an Intel PC from the 860 PRO, recorded

**Type:** porting (evidence) · **Owner:** `PRT`, with the maintainer · **Priority:** P1 · **Status:** open · **Blocked by:** the maintainer's PC and SSD ([issues-human](../issues-human/README.md#5-an-intel-pc-and-a-sata-ssd-for-the-first-real-x86-boot)) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.1, MC-12.2, MC-12.9

## Problem

No physical x86 machine is part of the evidence. The maintainer has a Samsung 860 PRO and an Intel PC to try.

## Plan

1. The first run follows 211's steps for today: written through a USB-SATA adapter, then the SSD on the first internal SATA port.
2. The maintainer sends back:
   - the PC's board or model;
   - the CPU;
   - the firmware version and the settings used (UEFI, CSM, Secure Boot, SATA mode, x2APIC);
   - a photo of the screen;
   - the serial output, if COM1 exists;
   - `cpus`, `svc`, `stat devices` and `physmap` from the shell, if it comes up.
3. Each stop found becomes a fix in 211-PRT-0002, 0003, 211-KRN-0012, 0013 or 211-DRV-0002, or a new issue.
4. Once it boots, a short checklist is run by hand and recorded:
   - `ls`;
   - a program;
   - `write data/x`, `sync`, `reboot`, then `cat data/x`;
   - the keyboard;
   - `cpus` (every core online).
5. The profile gains `x86-64/PC-0`, which names the machine, the firmware settings, the TCB (that firmware) and the checklist results. It is a separate configuration: QEMU evidence does not carry over (MC-12.1, MC-12.9).

## Acceptance criteria

The checklist passes on the maintainer's PC and is recorded in the profile with the machine and the firmware settings.

## Related

[211](211-intel-pc-from-a-sata-ssd.md).
