# 000-KRN-0124 — Switching a machine between Linux and MIND Core: one boot of MIND Core by `BootNext`, and back to Linux by any restart

**Type:** script (`scripts/boot_switch.py`), docs · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (in `main` at 1b1436a; the Ryzen's run left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-6.1, MC-9.3, MC-12.1, MC-12.9

## Problem

A run on real hardware needs the maintainer at the machine. He writes the stick on another computer, boots the PC from it through the firmware's boot menu, and brings the stick back to read `log:`. On the Ryzen, which also runs Ubuntu for Effector, the steps could be done from Linux instead:
- write the stick (`scripts/write_usb_linux.py`);
- boot MIND Core once for a run;
- come back to Linux to read the logs and write the next image.

The maintainer asked on 2026-10-20 whether our bootloader could switch between the two. No task covered it.

**The firmware already switches.** The bootloader needs no change:
- `BootNext` names the entry the next boot starts, once. The firmware deletes it before it starts that entry.
- `BootOrder` names every boot after that.

The kernel track already sets `BootNext` from MIND Core (`efivar bootnext`, [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done)). Linux sets it with `efibootmgr --bootnext`.

**Why not a choice in our bootloader** (a menu, or a variable it reads to start Linux's loader):
- it would change the signed boot chain (MC-9.1);
- a failure before our bootloader decided would leave the machine in MIND Core.

With `BootNext`, every way a run of MIND Core can end returns to Linux: `reboot`, a panic, the watchdog's reset after a freeze, a power cut, or a bootloader that refuses the volume.

## Plan

1. **`BootOrder` keeps Linux first.** The script never changes `BootOrder`. It refuses to switch when the entry Linux booted from (`BootCurrent`) is not first, and says how to fix that.
2. **`scripts/boot_switch.py`, run as root on Linux.** It has three commands.
   - **`status`:**
     - prints `BootCurrent`, `BootNext`, `BootOrder` and the entries;
     - names Linux's entry and MIND Core's;
     - names the MIND Core disk: the one whose first partition is the FAT volume labelled `MIND CORE` holding `MANIFEST`, and whose second is `MIND LOG`.
   - **`once [--disk DEV] [--reboot]`:**
     - finds the entry for that disk: `\EFI\BOOT\BOOTX64.EFI` on its first partition, with the image's MBR signature (vvfat's `0xBE1AFDFA`), start and size. If there is none, it creates one with `efibootmgr --create-only`, labelled `MIND Core`, so `BootOrder` is not touched;
     - sets `BootNext` to it and records the newest `BOOTNNNN.LOG` on `MIND LOG`;
     - with `--reboot`, restarts the machine.
     - It refuses:
       - a disk that holds a mounted Linux file system or the running root;
       - two candidate disks;
       - a Linux entry that is not first.
   - **`logs DIR`:**
     - mounts `MIND LOG` read-only in a temporary directory;
     - copies the boot logs and hardware reports newer than the ones recorded at `once`;
     - says whether MIND Core booted, from a new `BOOTNNNN.LOG` and its `[SHELL] AUTORUN` lines;
     - says when the firmware did not start the stick. Some firmware skips USB with Fast Boot on, or ignores `BootNext`.
3. **An unattended run.** The image's `autorun.msh` (000-APP-0079), written onto the stick after the image with `scripts/disk_copy.sh`:
   - runs what the run needs (`msh diag/full.msh`, `check`);
   - ends with `reboot`, under `requires: lifecycle`.

   So a run is:

   ```
   write_usb_linux.py
   → disk_copy.sh (proprietary firmware, autorun.msh)
   → boot_switch.py once --reboot
   → the run
   → reboot
   → boot_switch.py logs
   ```
4. **A freeze in MIND Core.**
   - On an Intel PC, the TCO watchdog (000-KRN-0076) resets the machine within 30 s, and that is a boot into Linux.
   - The Ryzen has no watchdog in MIND Core yet (`WATCHDOG: NONE (NO INTEL LPC BRIDGE)`), so a freeze there waits for a person. [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md) arms the AMD FCH's watchdog.
5. **Effector's machines** ([docs/effector.md](../docs/effector.md)).
   - The Ryzen (`un-rizen`) is a work machine: the gates' fastest x86 machine. Its rules forbid agents to reboot it, and a run of MIND Core takes it out of Effector until it is back in Linux.
   - So, until the maintainer records otherwise in docs/effector.md section 6:
     - only the maintainer runs `once --reboot` there, after checking that no operation is active;
     - an agent prepares the stick and reads the logs.
   - A machine of the rig ([000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md)) is a test machine, where this is its purpose.
6. **Docs.** [docs/write-disk.md](../docs/write-disk.md) and its Russian version get a section on switching from Linux. `--help` says what each command changes.

## Acceptance criteria

- [x] Host tests (`tests/boot_switch_test.py`, in `host_tests.sh`), with recorded `efibootmgr -v` outputs and `lsblk` trees:
  - the MIND Core entry is found by its MBR partition;
  - it is created when missing, with `--create-only`;
  - a Linux entry that is not first, two candidate disks, or a mounted disk are refused;
  - `BootOrder` is never written.
- [ ] On the Ryzen (Ubuntu 24.04), with the maintainer:
  - `once --reboot` starts MIND Core from the stick once;
  - its `reboot` comes back to Ubuntu;
  - `logs` copies that boot's log and says it booted.

  The run is recorded with its logs, for that machine and its firmware only (MC-12.1, MC-12.9).
- [x] docs/write-disk.md (EN and RU) describes the switch, what the script refuses, and that a firmware ignoring `BootNext` keeps booting Linux.

## Progress (2026-10-20)

- **`scripts/boot_switch.py`**: `status`, `once [--disk DEV] [--reboot]` and `logs DIR`, as the plan says.
  - It finds the stick's entry by its first partition's `HD(1,MBR,…)` node, or makes one with `--create-only`.
  - It stops before `BootNext` if making the entry changed `BootOrder`.
  - It says when Secure Boot is on.
- **`tests/boot_switch_test.py`** (in `host_tests.sh`) runs the script against a model of the firmware's variables, `lsblk` and the MBR:
  - an entry made and booted once;
  - an existing one used and turned active;
  - `BootOrder` never written;
  - every refusal leaving the variables as they were;
  - a create that moved `BootOrder` stopping before `BootNext`;
  - `logs` telling a boot of MIND Core from none.
- **Docs.** docs/write-disk.md and its Russian version, section 10 (version 1.4).
- **Left:** the run on the Ryzen with the maintainer.

**The gate (2026-10-20).** Gate g0131 passed (`c23e754` on Effector's machines; the two groups with the updater suite again on `1b1436a`), and `main` is at 1b1436a. The host tests ran in it.

## Related

[351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done) (`efivar`, `BootNext` from MIND Core), [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done) (each slot's own loader, which the one at `EFI/BOOT/` starts), [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (the watchdog), [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md), [000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md) (the rig), [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md) (`write_usb_linux.py`), [docs/write-disk.md](../docs/write-disk.md), [docs/effector.md](../docs/effector.md).
