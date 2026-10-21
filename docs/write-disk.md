# Writing MIND Core to a disk and booting a PC

**Version:** 1.4 (2026-10-20): one boot of MIND Core from Linux (section 10) · **Issues:** [211](../issues/211-intel-pc-from-a-sata-ssd.md) (an Intel PC from a SATA SSD), [211-PRT-0001](../issues/211-PRT-0001-writer-for-an-internal-disk.md) (the writer), [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md) (the first run), [211-PRT-0006](../issues/211-PRT-0006-log-partition-in-the-image.md) and [211-KRN-0019](../issues-done/211-KRN-0019-boot-logs-on-the-log-partition.done) (the log partition), [000-KRN-0124](../issues/000-KRN-0124-switching-between-linux-and-mind-core.md) (one boot from Linux) · **Russian version:** [write-disk_RU.md](write-disk_RU.md)

> **No physical x86 machine has run MIND Core as part of the evidence yet.** Everything below follows from the scripts and from QEMU, where the same image boots from USB, SATA (AHCI) and NVMe. The first real run is task [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md); section 8 lists what is known to be risky on a real PC and what to send back.

## 1. What you need

- **A Linux machine** with Python 3, util-linux (`lsblk`, `findmnt`, `blockdev`) and `sudo`, plus the project's toolchain (`./01_prepare_env.sh`) and `qemu-img` (comes with QEMU) to build the image.
- **A disk whose contents may be lost**, of 1 GiB or more (the image is 568 MiB). Any of these:
  - a USB stick;
  - an SSD in a USB-SATA adapter or enclosure (for example a Samsung 860 PRO);
  - an SATA or NVMe disk inside the Linux machine, with `--internal` (section 4). It must not be the disk Linux runs from: the writer refuses that one in every case.
- **The PC to boot:** x86-64 with UEFI. Legacy BIOS boot is not supported.

## 2. Build the image

```bash
./04_make_usb_image.sh --force
```

It builds everything and writes `dist/mind-core-usb.img` (568 MiB). Its MBR has two partitions:

- **`MIND CORE`**, the boot volume: FAT16, marked as the EFI system partition. It holds `EFI/BOOT/BOOTX64.EFI`, the kernel, the services and the programs.
- **`MIND LOG`**, the log volume: 64 MiB of FAT16 with an ordinary type, which Windows, macOS and Linux mount by themselves. MIND Core writes the system log of each boot there as `BOOTNNNN.LOG` (section 9).

The script checks every file it packed and both partitions, and prints the image's SHA-256. `--force` replaces an older image; without it an existing image is kept.

With `--slots` the kernel, the services and the signed manifest go into slot A (`MIND/A/`), confirmed in `MIND/BOOT0`, so that the updater can fill slot B ([docs/update/updater.md](update/updater.md)). `--channel FILE` keeps a release's signed channel file in slot A, and then the updater knows the running version; its manifest must be this build's. `--update FILE` puts an `update.txt` at the root.

Optional: boot exactly this image in QEMU first, as a USB stick:

```bash
python3 tests/usb_image_smoke.py --qemu qemu-system-x86_64 --firmware OVMF.fd
```

## 3. Find the disk

```bash
./05_write_usb_linux.sh --list              # USB disks
./05_write_usb_linux.sh --list --internal   # also SATA and NVMe disks inside this machine
```

Each line shows the path, size, model, serial number and bus. A disk that holds Linux's own file systems is marked `SYSTEM DISK: REFUSED`. Use the stable `/dev/disk/by-id/…` name of the whole disk (`ls -l /dev/disk/by-id/`), not a partition such as `/dev/sdb1`.

## 4. Check without writing

```bash
./05_write_usb_linux.sh --device /dev/disk/by-id/usb-Samsung_SSD_860_PRO_…  --check
./05_write_usb_linux.sh --device /dev/disk/by-id/ata-Samsung_SSD_860_PRO_…  --internal --check
```

`--check` needs no root and writes nothing. It shows the image, its SHA-256, the disk, and every partition on it with its file system, label and mount point: everything that the write would destroy.

## 5. Write: the script asks before it writes

```bash
./05_write_usb_linux.sh --device /dev/disk/by-id/…            # a USB disk
./05_write_usb_linux.sh --device /dev/disk/by-id/… --internal # an internal SATA or NVMe disk
```

The script asks, in this order:

1. **`sudo` password.** If you did not start it as root, it runs itself again with `sudo`.
2. **The disk.** It prints `ALL DATA ON THIS DISK WILL BE LOST:` with the disk's model, serial number, size and bus, and every partition on it.
3. **Prompt `1/2`:** type the disk's serial number. If the disk reports none, type its model, or else its device name; the expected text is shown in brackets. This confirms that you picked this disk and not another.
4. **Prompt `2/2`:** type `ERASE /dev/sdX`, with the disk's own path as the prompt shows it.

Any other answer to either prompt stops the script, and nothing is written.

Then it:

- checks again that the disk is still the same one, and unmounts its file systems;
- opens it exclusively and writes the image, showing the progress;
- clears old GPT data at the end of the disk;
- reads the image back and compares its SHA-256.

`Done. SHA256 matches.` means the disk is ready. A cancelled or failed write can leave a partial image: run the writer again.

The writer refuses, whatever you answer:

- the disk Linux runs from: one holding `/`, `/boot`, `/boot/efi`, `/usr`, `/var` or `/home`;
- the disk holding the image or the repository;
- a disk used by LVM, RAID or encryption, or holding active swap;
- a read-only disk, a disk smaller than the image, or one with logical sectors other than 512 bytes. Some USB-SATA adapters report 4096-byte sectors; use another adapter or `--internal`.

The rest of a disk larger than the image stays unused.

On Windows, `05_write_usb_windows.ps1` writes USB disks only (README, "Write the image from Linux or Windows").

**From WSL on Windows, in one step:** `scripts/build_and_write.sh <disk number>`.
- It pulls `fast-test` (or builds `--commit <id>`), builds, and packs the image.
- It opens an elevated PowerShell running `05_write_usb_windows.ps1`, which asks before it writes (`ERASE <number>`).
- After the write it copies the proprietary files and the desktop pictures onto the disk, and the test files its options ask for: `--link-dev`, `--keep-wifi`, `--wifi-ofdm` and others, listed in the script's header.
- **`--slots`** writes an image with boot slots and an `update.txt` that takes releases from `data/link/rel/`. After that one write, `scripts/link_update.py` updates the machine over the link (351-KRN-0141).
- `--list` shows the USB disks and their numbers.
- It builds the repository it lives in, also when it is started through a symbolic link from outside that repository.

## 6. Boot the PC

**Firmware settings** (in the setup screen, usually Del, F2 or F10 at power-on):

- boot mode **UEFI**; CSM or "Legacy" **off**;
- **Secure Boot off**: the bootloader is not signed yet (issue [350](../issues/350-signed-boot-images.md));
- SATA mode **AHCI**, not RAID, Intel RST or VMD;

**The disk:**

- either inside the PC on its first SATA port (`SATA0`/`SATA1` on the board);
- or still in its USB adapter on a rear USB port.

For the first test, disconnect the PC's other disks and USB sticks: the programs may be read from another disk's FAT volume (section 8).

**Start it** from the firmware's boot menu (often F8, F11 or F12) by choosing the disk's UEFI entry, or by putting it first in the boot order.

**An Intel Mac** (for example a MacBook Pro of 2012–13; not tried yet, [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md)):

- **Starting it.** Attach the disk by USB, hold Option (⌥) at power-on and choose its `EFI Boot` entry.
  - Macs before the T2 chip have nothing to change in the firmware.
  - A Mac with T2 (from 2018) needs, in Startup Security Utility, "No Security" and "Allow booting from external media".
- **Restarting into MIND Core.** The choice made with Option holds for that boot only. Any restart starts macOS again, `mindlink reboot` included: seen on the MacBook Pro on 2026-10-21.
  - **To make the disk the default startup disk,** run this once in the Terminal of macOS Recovery, with the disk attached. macOS itself refuses it (below).
    - macOS mounts only the log partition, `MIND LOG`. The boot partition is marked as the EFI system partition (type `0xEF`), which macOS leaves unmounted, so it is mounted by hand first.
    - `MIND LOG`'s disk is the stick: `disk2`, say, and the boot partition is its first, `disk2s1`.

    ```bash
    diskutil info "/Volumes/MIND LOG" | grep "Part of Whole"     # the stick, e.g. disk2
    sudo mkdir -p /Volumes/MINDCORE
    sudo mount -t msdos /dev/disk2s1 /Volumes/MINDCORE           # its boot partition
    sudo bless --mount /Volumes/MINDCORE --setBoot --file /Volumes/MINDCORE/EFI/BOOT/BOOTX64.EFI --shortform
    bless --getBoot                                              # the device the firmware starts now
    sudo umount /Volumes/MINDCORE
    ```

    `bless` records the disk in the firmware's settings (NVRAM). The Mac should then start it at every power-on and restart while it is attached, and macOS when it is not.
  - **Where it works: macOS Recovery.** Hold Command-R at power-on, then Utilities, Terminal. It runs as root, so no `sudo`. The disk numbers there may differ: find the stick with `diskutil list`.
    - **On the MacBook Pro on 2026-10-21 it worked from Recovery.**
    - After it, `mindlink reboot` came back into MIND Core in about 9 s, with no key pressed, and the Wi-Fi joined again.
  - **From macOS itself it is refused.** With the partition mounted, `bless` answered `Could not set boot device property: 0xe00002e2`. That is `kIOReturnNotPermitted`, from macOS's System Integrity Protection (from 10.11), and `bless --getBoot` still named macOS's volume.
  - **If a rewritten disk is not started,** run the command again.
  - **To make macOS the default again:** System Preferences (System Settings), Startup Disk, macOS's disk.
- **The internal disk.** It keeps its own EFI partition:
  - the bootloader reads only the disk it was started from ([211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done));
  - `vfs_server` mounts only FAT volumes in an MBR partition table, so it skips the internal disk's GPT.
- **No COM1.** Two things show what happened instead:
  - the bootloader's progress and errors, in text ([211-KRN-0015](../issues-done/211-KRN-0015-boot-errors-on-a-mac-screen.done), [211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done));
  - the kernel's boot lines and stops ([211-KRN-0013](../issues-done/211-KRN-0013-fatal-messages-on-the-screen.done)).
- **The keyboard and trackpad.** They are USB devices inside the Mac. On Intel 7–9 series chipsets the kernel moves the USB ports from the EHCI controllers to the xHCI one before `usb_host` starts.

## 7. What a good boot looks like, and what to check

The bootloader loads the kernel and the services from the disk, the screen switches to MIND Core, and the shell's prompt `MIND>` appears. Then:

```
cpus                       # every core online
svc                        # services running; ahci (internal disk) or usb_storage (USB) among them
ls                         # the programs on the disk
run clock                  # a program (Esc ends it)
write data/hello first boot
sync
reboot
cat data/hello             # after the reboot: the file survived
ls log:                    # the log partition: a BOOTNNNN.LOG for each boot
```

## 8. If it stops

| What you see | Likely cause | What to do |
|---|---|---|
| `BOOT ERROR: display: …` | The firmware gives no linear framebuffer (GOP) | Another video output, or the integrated graphics; report it |
| `BOOT ERROR: kernel.elf: …`, `BOOT ERROR: <name>.elf: …` or `BOOT ERROR: boot volume: …` | A file on the disk the bootloader started from is missing or damaged, or the firmware shows no file system on it. The bootloader reads only its own disk ([211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done)) | Write the disk again (section 5) |
| The shell runs, but programs do not start or `ls` is empty; `logs` of `vfs_server` (its PID from `ps`) says `THE BOOT VOLUME (…) IS ON NO BLOCK DEVICE` | `vfs_server` mounts only the volume the bootloader was read from, and no driver shows that disk (for example a controller MIND Core has no driver for) ([211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done)) | Attach the disk to another port or bus (USB on a rear port, the first SATA port) |
| Grey `MIND CORE KERNEL: …` lines, then white text on dark red: `KERNEL PANIC`, `KERNEL EXCEPTION` or `INIT EXITED` | The kernel stopped, and the red text says why ([211-KRN-0013](../issues-done/211-KRN-0013-fatal-messages-on-the-screen.done)). | Photograph the screen |
| The firmware's picture (a Mac's spinner) stays, and no `MIND CORE BOOT:` line appears | The firmware did not start the bootloader, or its console did not switch to text | Report it, with the boot menu entry you chose |
| `MIND CORE BOOT:` lines end at `STARTED; READING …` | The bootloader stopped while the firmware read the files from the disk | Another USB port or adapter; report it with a photo |
| `MIND CORE BOOT:` lines end at `… EXITING BOOT SERVICES`, and nothing from the kernel follows | The kernel stopped before it took the screen, or the GOP the bootloader chose (`USING GOP`) is not the screen's; the `GOP` lines list every one the firmware has ([211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done)) | Photograph the lines. A serial cable on COM1, if the board has one, shows the reason |
| The shell runs, but `ls` is empty or `[INIT] ahci NOT STARTED` | SATA is in RAID/RST/VMD mode, or the disk is not on the first SATA port ([211-DRV-0002](../issues/211-DRV-0002-ahci-every-port.md)) | AHCI mode; the first port |
| `MIND CORE KERNEL: NO TICK FROM THE PIT`, or everything waits forever (programs that sleep, time not moving) | No timer interrupt reaches the kernel. The line `MIND CORE KERNEL: TICK: …` says where the tick comes from: the LAPIC timer, or the PIT where the firmware lists no ACPI PM timer ([211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done)) | Photograph the lines and report it |
| The keyboard does nothing | Only PS/2 and USB keyboards on the first USB 3 (xHCI) controller work; on Intel 7–9 series chipsets the kernel first moves the USB ports there from EHCI | Another USB port (rear, on the chipset) or a PS/2 keyboard |

**Pauses for a photo of the screen.** The boot does not wait by default. To photograph the boot screens, put a number of seconds (at most 60) in `data/pause.txt` on the boot volume: from Windows on the written stick, or in MIND with `write data/pause.txt 5` and a restart. Then three screens stay that long:

- the firmware's lines (`A PAUSE OF 5 S: THE LINES ABOVE ARE THE FIRMWARE'S`);
- the bootloader's `MIND CORE BOOT:` lines (`A PAUSE OF 5 S FOR A PHOTO OF THESE LINES`);
- the kernel's grey lines, before the compositor takes the screen.

`0` or no file: no pause (211-KRN-0089). A boot that fails stops with its error on the screen anyway.

## 9. What to send back

For [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md), whether it worked or not:

- the PC's board or model, the CPU, and the firmware version;
- the firmware settings you used (section 6);
- how the disk was attached: internal SATA port, or USB adapter;
- **the files of the `MIND LOG` partition.** Attach the disk to any computer and copy `BOOTNNNN.LOG` from it:
  - each file is one boot's system log, written every 2 seconds, so it reaches the last seconds before a hang;
  - `HWNNNN.TXT` beside it is the same boot's hardware report (174-KRN-0038): the CPU, the firmware's memory map and ACPI tables, every PCI function, and the kernel's own lines (`MIND CORE KERNEL: …`), those the screen lost included;
  - the folder `ACPI` holds the firmware's ACPI tables as files (`DSDT.bin`, `FACP.bin`, …);
- a photo of the screen where it stopped, or the output of `cpus`, `svc`, `stat devices` and `physmap` if the shell came up;
- the serial output, if you had a COM1 cable.

The profile records the machine as a configuration of its own: what was run on it, with which firmware settings (MC-12.1).

## 10. One boot of MIND Core from Linux, and back

On a PC that runs Linux from its own disk, MIND Core can be started once from the stick without the firmware's boot menu ([000-KRN-0124](../issues/000-KRN-0124-switching-between-linux-and-mind-core.md)). Every restart after that boot comes back to Linux. On the Ryzen, which runs Ubuntu, a run looks like this:

```bash
./05_write_usb_linux.sh --device /dev/disk/by-id/…   # section 5
sudo scripts/boot_switch.py status                   # the boot entries, Linux's first, and the MIND Core disk
sudo scripts/boot_switch.py once --reboot            # the next boot starts the stick, once
# … the run in MIND Core; its reboot, a panic, the watchdog or a power cut brings Linux back …
sudo scripts/boot_switch.py logs ~/mind-runs         # the MIND LOG volume copied, read-only
```

**How it works.** The firmware has two variables:
- **`BootNext`** names the entry the next boot starts, and the firmware deletes it as that boot begins;
- **`BootOrder`** names every boot after that.

`once` sets `BootNext` to the stick's entry. If the stick has no entry, `once` makes one first with `efibootmgr --create-only`, labelled `MIND Core`. Nothing in the image or the bootloader changes.

**What the script refuses**, saying why:
- Linux's entry (`BootCurrent`) is not first in `BootOrder`: a restart would not come back. Put it first in the firmware's setup.
- Linux was not started by UEFI.
- No MIND Core disk is attached, or several are (name one with `--disk`).
- The MIND Core disk is mounted.

The script never writes `BootOrder`.

**`logs`** copies the whole `MIND LOG` volume into a new directory, then says one of:
- MIND Core booted since `once`, which new `BOOTNNNN.LOG` files it wrote, and whether `autorun.msh` ended;
- it did not boot. The firmware may skip USB disks with Fast Boot on, or ignore `BootNext`; Secure Boot with Microsoft's keys refuses the bootloader (section 6).

**An unattended run.** Put an `autorun.msh` at the stick's root after writing it. The shell runs it once at start ([docs/msh.md](msh.md)), and it can end with `reboot`. For example, `check` writes its full log to `log:`:

```
#!msh
requires: files sysinfo log lifecycle
check
reboot
```

`diag/full.msh` copied as `autorun.msh`, with `reboot` added as its last line, does the full hardware run. A step that asks the user (the camera, the firmware's variables) waits for an answer, so leave such steps out of a run with no one at the machine.

**A freeze.**
- On an Intel PC, the chipset's watchdog restarts the machine within 30 s, and that restart is a boot into Linux.
- On the Ryzen the AMD FCH's watchdog does the same ([213-KRN-0125](../issues/213-KRN-0125-the-amd-fch-watchdog.md)). It is checked against a model of its registers only, so the first runs show whether it holds: the boot line says `WATCHDOG: AMD FCH, A RESET AFTER 30 S ...` when it is armed. If it says `WATCHDOG: NONE`, a freeze waits for the power button.

**Effector's machines.** On a machine Effector works on (the Ryzen is one), only the maintainer runs `once --reboot`, after checking that no operation is active ([docs/effector.md](effector.md)).
