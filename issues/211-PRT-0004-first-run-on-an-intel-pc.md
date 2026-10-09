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

## Progress

**2026-10-08: a MacBook Pro Retina 15" (A1398, 2012–13, Intel and NVIDIA GPUs) boots to the shell and `wm`.**

The image was written to the 860 PRO in a USB-SATA adapter (`7825:A2A4`) and started with Option, then "EFI Boot". The firmware was Apple's, with its defaults: no CSM, no Secure Boot to turn off. There is no COM1, so every step was read from photographs of the screen. The stops, in order, each fixed by a task:

1. **A frozen firmware spinner after "EFI Boot".**
   - The bootloader took the first GOP and the first file system the firmware listed, and its errors could not show on a Mac's console.
   - Fixed by [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done) (its bootloader part), [211-KRN-0015](../issues-done/211-KRN-0015-boot-errors-on-a-mac-screen.done) and [211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done).
2. **`INIT STARTED`, then nothing.**
   - The PIT counts there, but its interrupt never reaches the CPU, so the kernel waited for a first tick forever.
   - [211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done): the tick now comes from the LAPIC timer. The boot line reads `TICK: LAPIC TIMER, 62357 PER TICK, MEASURED ON THE ACPI PM TIMER; TSC 2294 MHZ; PIT COUNTING`.
3. **The services' lines stopped at the compositor.** [211-KRN-0017](../issues-done/211-KRN-0017-logs-on-the-boot-screen.done) now draws them until the compositor's first frame.
4. **`[COMPOSITOR] NO MEMORY FOR SHADOW`.** At 2880 × 1800 the compositor's shadow did not fit its quota: [211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done).
5. **The shell ran, but no key reached it.** A stalled `SET_IDLE` halted endpoint 0 of the Logitech receiver: [211-DRV-0003](../issues-done/211-DRV-0003-usb-host-on-real-hardware.done).

Seen on the machine:

- **Disks.** `ahci` finds the internal SSD on port 0 (490 234 752 sectors). `vfs_server` mounts the FAT16 volume from USB and skips the internal disk's GPT.
- **USB.**
  - The kernel moves USB ports from EHCI to xHCI.
  - The xHCI controller has 8 ports. The adapter runs at SuperSpeed on port 5 and the receiver at full speed on port 2. Hot plug works.
- **Display.** The display runs at 2880 × 1800. The shell, `wm` and `dzen-clock` draw correctly.
- **`devices`.** Intel HD 4000 (00:02.0) and an NVIDIA GPU (01:00.0), HDA audio on both, AHCI (00:1f.2), xHCI (00:14.0), two EHCI (00:1a.0, 00:1d.0), a Broadcom Ethernet and card reader (03:00.0, 03:00.1), Wi-Fi (04:00.0), Thunderbolt bridges (05:00.0, 06:xx) and the MEI (00:16.0). No driver for the GPUs, network, Wi-Fi or Thunderbolt.

Open:

- the internal keyboard and trackpad ([211-DRV-0004](211-DRV-0004-ehci.md));
- the checklist of step 4;
- the profile entry for this machine.

The shell's log view and heartbeat ("UP n S") before the first key, and `usb_host`'s new log lines, are diagnostics added for this run.

## Related

[211](211-intel-pc-from-a-sata-ssd.md).
