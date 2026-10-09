# 211 — A PC with an Intel CPU, booted from a SATA SSD: the first real x86 machine

**Type:** porting (main task) · **Owner:** `PRT` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track H, stage I (a profile for a real platform) · **Constitution:** MC-9.1, MC-12.1, MC-12.9

Asked by the maintainer (2026-10-08). They have a Samsung 860 PRO (a SATA SSD) and want the system on it, to check a boot on an Intel PC first.

## Problem

Nothing in the profile has run on a physical x86 machine: "physical machines boot from the USB image but are not part of the evidence" (`docs/profile/README.md`), and no machine is named anywhere. What exists today:

**Ready**

- `04_make_usb_image.sh` builds a 504 MiB image: MBR, one FAT16 partition of type 0xEF with `EFI/BOOT/BOOTX64.EFI`, the kernel, the services and the programs.
- `05_write_usb_linux.sh` writes it and verifies it by reading back, but only to a disk whose transport is USB (`scripts/write_usb_linux.py:40`). An 860 PRO in a USB-SATA enclosure or adapter is accepted; on an internal SATA port it is refused, and so it is by the Windows writer.
- After boot, `ahci` reads and writes a SATA disk in AHCI mode (READ/WRITE DMA EXT, FLUSH CACHE EXT), and `vfs_server` mounts the 0xEF partition. Files in `data/` persist. This is tested only in QEMU.

**Risky on a real PC**

- **x2APIC.** The kernel stops if the firmware left the local APIC in x2APIC mode: `assert … "x2APIC is not supported yet"`, `kernel/src/arch/x86_64/cpu.rs:78`. Many recent Intel boards do.
- **The tick.** It comes from the 8254 PIT through the 8259 into the boot CPU's LAPIC. Some recent chipsets gate the PIT; then the tick stops. This is an assumption to check, not something the repository shows.
- **The boot volume.** The bootloader opens the first file system the firmware lists, not its own device (`bootloader/src/main.rs:151`). `vfs_server` mounts the first FAT volume its block drivers find. With a second disk or a USB stick attached, the two can differ.
- **Which disk.** `ahci` takes only the first port with a disk on the first controller (`ahci/src/main.rs:34`).
- **Where messages go.** Kernel messages and panics go only to COM1. A PC without a serial port shows a frozen screen and no reason.
- **The firmware's mode.** In RAID, Intel RST or VMD mode the controller is not class 01:06:01, so `ahci` does not start and programs cannot be loaded.

**Missing**

- No installer inside the system.
- No TRIM, no GPT, no separate data partition, no HPET or I/O APIC.
- Secure Boot must be off: the bootloader is unsigned (350).

## What the maintainer can do now

The full steps are in [docs/write-disk.md](../docs/write-disk.md) ([Russian](../docs/write-disk_RU.md)).

1. Put the 860 PRO in a USB-SATA enclosure or adapter on a Linux machine.
2. Run `./04_make_usb_image.sh --force`, then `./05_write_usb_linux.sh --list`, then `./05_write_usb_linux.sh --device /dev/disk/by-id/usb-…`, which asks for `sudo` and twice for confirmation.
3. Connect the SSD to the PC's first SATA port. Set the firmware to UEFI with CSM off, Secure Boot off, SATA mode AHCI, and x2APIC off if the firmware offers the switch. Disconnect other disks and sticks.
4. If the board has a COM1 header, a serial cable shows the kernel's lines.

These are the steps of 211-PRT-0004, which records the result.

## Plan: tasks by track

| Task | Track | What |
|---|---|---|
| [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md) | `PRT` | The image writer for an internal SATA or NVMe disk, behind an explicit option and the same refusals of system disks |
| [211-PRT-0005](../issues-done/211-PRT-0005-windows-writer-default-image.done) | `PRT` | The Windows writer's default image under `powershell -File` (Windows PowerShell 5.1) |
| [211-PRT-0002](../issues-done/211-PRT-0002-x2apic.done) | `PRT` | The local APIC in x2APIC mode, as firmware leaves it |
| [211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done) | `PRT` | A tick that does not depend on the 8254: the LAPIC timer, with the PIT only where it counts |
| [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done) | `KRN` | The bootloader loads from its own device and names that volume in `BootInfo`; `vfs_server` mounts that one |
| [211-KRN-0013](../issues-done/211-KRN-0013-fatal-messages-on-the-screen.done) | `KRN` | The kernel's boot line and fatal messages on the screen too, not only on COM1 |
| [211-KRN-0015](../issues-done/211-KRN-0015-boot-errors-on-a-mac-screen.done) | `KRN` | Bootloader errors and panics readable on a Mac's screen (Apple's console control in text mode) |
| [211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done) | `KRN` | The bootloader takes the screen's GOP (a console output's), not the first listed, and prints its progress on the text console |
| [211-KRN-0017](../issues-done/211-KRN-0017-logs-on-the-boot-screen.done) | `KRN` | Service logs on the boot screen until the compositor's first frame |
| [211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done) | `KRN` | The compositor's memory quota fits the screen |
| [211-DRV-0002](211-DRV-0002-ahci-every-port.md) | `DRV` (open) | `ahci`: every port with a disk and every controller; the boot disk found by 211-KRN-0012's identity |
| [211-DRV-0003](../issues-done/211-DRV-0003-usb-host-on-real-hardware.done) | `DRV` (open) | `usb_host` on real hardware: endpoint 0 after a stall; what failed is logged |
| [211-DRV-0004](211-DRV-0004-ehci.md) | `DRV` (open) | An EHCI driver for an Intel Mac's internal keyboard and trackpad (proposed) |
| [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md) | `PRT`, with the maintainer | The first run on the maintainer's PC and 860 PRO, recorded; the profile gets the machine as its own configuration |

Later, as their own issues when a run shows the need:

- TRIM;
- RST and VMD;
- the HPET and the I/O APIC;
- a GPT disk with a data partition;
- the network, on the MacBook Pro first: [550](550-network-on-real-hardware.md).

Installing from inside the running system comes with self-update ([351](351-self-update.md)), which writes boot slots.

## Acceptance criteria

The maintainer's Intel PC boots the image from the 860 PRO on an internal SATA port to the shell; `ls`, a program and a file written in `data/` work after a reboot. The profile names the machine, its firmware settings and what was run on it, as a configuration of its own (MC-12.1).

## Related

[350](350-signed-boot-images.md), [351](351-self-update.md), [issues-human](../issues-human/README.md#5-an-intel-pc-and-a-sata-ssd-for-the-first-real-x86-boot).
