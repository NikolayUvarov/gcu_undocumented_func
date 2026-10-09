# 211-KRN-0019 — Each boot's system log on the log partition (`log:`)

**Type:** kernel services · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-10.6, MC-12.1

## Problem

`logd` keeps the system log in RAM: 256 records, lost at a reset or a hang. On a machine without a serial port, the record of a boot is whatever was on the screen when it was photographed. That is how the MacBook Pro of [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md) is debugged today.

## Plan

- **`log:`.** `vfs_server` mounts the boot disk's FAT partition labelled `MIND LOG` ([211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md)) as the volume `log`.
  - The boot volume and the log volume share the disk's one block client and its cache.
  - The shell's client writes anywhere on `log:`, as on `ram:`; applications read it.
  - `libmind::fs` knows the volume name.
- **The boot log.** `vfs_server` writes the system log of each boot to `log:bootNNNN.log`:
  - the number follows the highest one already there;
  - logs older than the last 50 are removed;
  - the file starts with a header and the machine's clock.
- **Saving.**
  - Every 2 s, between requests, `vfs_server` reads the new records from `logd` and appends them, then flushes the volume.
  - Records the ring dropped before they were read are counted in the file.
  - When less than 4 MiB of the volume would stay free, the oldest boot logs are removed. If that is not enough, the log stops with a line saying so.
- **The right to read the log.** `init` gives `vfs_server` the log client with the read badge, as it gives the shell.
  - `logd` never calls `vfs_server`, so there is no cycle of calls.
  - `vfs_server` already holds every file of the system.

Not in this task:

- the kernel's own boot lines (the PCI table, the tick, the CPUs) are not in `logd`; carrying them there is a later step;
- `logd`'s ring stays at 256 records.

## Acceptance criteria

- **QEMU, x86 and aarch64** (`tests/usb_image_smoke.py`): `vfs_server` mounts `log:` from the image; a line written with `logger` appears in this boot's file within seconds; a file written with `write log:…` and the boot log are read on the host.
- **The MacBook Pro:** the boot log of a run is read on another computer.

## Related

[211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md), issue 066 (FAT writing), [requests-APP.md](requests-APP.md) (`log:` in the shell's help and in `fm`).
