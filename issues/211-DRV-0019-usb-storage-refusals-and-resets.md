# 211-DRV-0019 — `usb_storage`: refusals explained, a lost device and a reset logged

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-4.8, MC-6.6, MC-12.1

## Problem

The maintainer's MacBook Pro run of 2026-10-09 (`fast-test` 57242b9, the boot disk in a USB-SATA adapter `7825:A2A4`) unplugged the boot disk for about 30 s and plugged it back, twice.

- In the second boot the log went on after the plug.
- In the first, the log volume afterwards held the image's original root plus a `boot0001.log` made anew, without its header. `hw0001.txt` and that boot's `acpi/` were gone, and so was everything logged before the unplug.
- The same scenario in QEMU (`device_del`, 30 s, `device_add`) keeps everything.

So the real device behaved in a way the log cannot show:

- `usb_storage` took any failed command for a lost interface. It claimed the interface again (up to 2 s of tries), checked the capacity and repeated the command, all without a word.
- A command the device refused (CHECK CONDITION) was never explained: no REQUEST SENSE, no log. A device that refuses SYNCHRONIZE CACHE made every flush spend 2 s and fail.
- A reset the device reports (UNIT ATTENTION 29h) was taken in silently. Its client, `vfs_server`, could not learn that writes the device had not made durable may be lost.

## Plan

- **A BOT cycle ends in one of three ways:** done, refused (CSW status 1, the device is there) or gone (a transport failure or a phase error).
- **A refusal** is followed by REQUEST SENSE and logged once per command and sense: key, ASC, ASCQ and their meaning. After a unit attention the command is repeated once. A refusal is not taken for a lost device.
- **A lost device** is logged once; claimed again, its return is logged too. A reset seen on its return is logged with a warning that writes not yet durable may be lost, and so is another medium found in its place.
- **Next steps, own issues when the run shows the need:**
  - telling `vfs_server` of a reset through `idl/block.wit` (a new version, and its use is the kernel track's);
  - a medium identity stronger than the capacity;
  - a QEMU suite with a hot unplug and plug of the boot disk (`ASR`).

## Acceptance criteria

- In QEMU, a hot unplug and plug of the USB boot disk logs the loss, the return and the reset (QEMU's disk reports a unit attention after it is plugged), and the system goes on writing its log.
- The next MacBook Pro run with a hot plug says in its log what the adapter answered.

## Related

[211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the boot log on `log:`), `usb_storage/src/main.rs`, `idl/block.wit`.
