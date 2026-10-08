# Writing MIND Core to a disk and booting a PC

**Version:** 1.2 (2026-10-08) · **Issues:** [211](../issues/211-intel-pc-from-a-sata-ssd.md) (an Intel PC from a SATA SSD), [211-PRT-0001](../issues/211-PRT-0001-writer-for-an-internal-disk.md) (the writer), [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md) (the first run) · **Russian version:** [write-disk_RU.md](write-disk_RU.md)

> **No physical x86 machine has run MIND Core as part of the evidence yet.** Everything below follows from the scripts and from QEMU, where the same image boots from USB, SATA (AHCI) and NVMe. The first real run is task [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md); section 8 lists what is known to be risky on a real PC and what to send back.

## 1. What you need

- **A Linux machine** with Python 3, util-linux (`lsblk`, `findmnt`, `blockdev`) and `sudo`, plus the project's toolchain (`./01_prepare_env.sh`) and `qemu-img` (comes with QEMU) to build the image.
- **A disk whose contents may be lost**, of 512 MiB or more. Any of these:
  - a USB stick;
  - an SSD in a USB-SATA adapter or enclosure (for example a Samsung 860 PRO);
  - an SATA or NVMe disk inside the Linux machine, with `--internal` (section 4). It must not be the disk Linux runs from: the writer refuses that one in every case.
- **The PC to boot:** x86-64 with UEFI. Legacy BIOS boot is not supported.

## 2. Build the image

```bash
./04_make_usb_image.sh --force
```

It builds everything and writes `dist/mind-core-usb.img` (about 504 MiB): an MBR with one FAT16 EFI system partition labelled `MIND CORE`. It holds `EFI/BOOT/BOOTX64.EFI`, the kernel, the services and the programs. The script checks every file it packed and prints the image's SHA-256. `--force` replaces an older image; without it an existing image is kept.

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
- **The internal disk.** It keeps its own EFI partition:
  - the bootloader reads only the disk it was started from ([211-KRN-0012](../issues/211-KRN-0012-boot-volume-identity.md));
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
```

## 8. If it stops

| What you see | Likely cause | What to do |
|---|---|---|
| `BOOT ERROR: display: …` | The firmware gives no linear framebuffer (GOP) | Another video output, or the integrated graphics; report it |
| `BOOT ERROR: kernel.elf: …`, `BOOT ERROR: <name>.elf: …` or `BOOT ERROR: boot volume: …` | A file on the disk the bootloader started from is missing or damaged, or the firmware shows no file system on it. The bootloader reads only its own disk ([211-KRN-0012](../issues/211-KRN-0012-boot-volume-identity.md)) | Write the disk again (section 5) |
| The shell runs, but `ls` shows another disk's files | `vfs_server` mounted the first FAT volume with an MBR partition table, which may be on another disk ([211-KRN-0012](../issues/211-KRN-0012-boot-volume-identity.md)) | Disconnect the other disks and sticks |
| Grey `MIND CORE KERNEL: …` lines, then white text on dark red: `KERNEL PANIC`, `KERNEL EXCEPTION` or `INIT EXITED` | The kernel stopped, and the red text says why ([211-KRN-0013](../issues-done/211-KRN-0013-fatal-messages-on-the-screen.done)). | Photograph the screen |
| The firmware's picture (a Mac's spinner) stays, and no `MIND CORE BOOT:` line appears | The firmware did not start the bootloader, or its console did not switch to text | Report it, with the boot menu entry you chose |
| `MIND CORE BOOT:` lines end at `STARTED; READING …` | The bootloader stopped while the firmware read the files from the disk | Another USB port or adapter; report it with a photo |
| `MIND CORE BOOT:` lines end at `… EXITING BOOT SERVICES`, and nothing from the kernel follows | The kernel stopped before it took the screen, or the GOP the bootloader chose (`USING GOP`) is not the screen's; the `GOP` lines list every one the firmware has ([211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done)) | Photograph the lines. A serial cable on COM1, if the board has one, shows the reason |
| The shell runs, but `ls` is empty or `[INIT] ahci NOT STARTED` | SATA is in RAID/RST/VMD mode, or the disk is not on the first SATA port ([211-DRV-0002](../issues/211-DRV-0002-ahci-every-port.md)) | AHCI mode; the first port |
| `MIND CORE KERNEL: NO TICK FROM THE PIT`, or everything waits forever (programs that sleep, time not moving) | No timer interrupt reaches the kernel. The line `MIND CORE KERNEL: TICK: …` says where the tick comes from: the LAPIC timer, or the PIT where the firmware lists no ACPI PM timer ([211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done)) | Photograph the lines and report it |
| The keyboard does nothing | Only PS/2 and USB keyboards on the first USB 3 (xHCI) controller work; on Intel 7–9 series chipsets the kernel first moves the USB ports there from EHCI | Another USB port (rear, on the chipset) or a PS/2 keyboard |

## 9. What to send back

For [211-PRT-0004](../issues/211-PRT-0004-first-run-on-an-intel-pc.md), whether it worked or not:

- the PC's board or model, the CPU, and the firmware version;
- the firmware settings you used (section 6);
- how the disk was attached: internal SATA port, or USB adapter;
- a photo of the screen where it stopped, or the output of `cpus`, `svc`, `stat devices` and `physmap` if the shell came up;
- the serial output, if you had a COM1 cable.

The profile records the machine as a configuration of its own: what was run on it, with which firmware settings (MC-12.1).
