# 000-DRV-0115 — `usb_storage` tries a refused READ or WRITE again, as Linux does

**Type:** driver (`usb_storage`) · **Owner:** `DRV` · **Priority:** P1 (the Ryzen's self-update failed on it twice) · **Status:** in progress (2026-11-27: the change and its host cases on the branch; not run on the Ryzen) · **Blocked by:** — · **Main task:** — · **Roadmap:** the drivers track · **Constitution:** MC-6.6, MC-12.1, MC-12.2, MC-12.3

## Problem

The Ryzen's boot drive refused a write twice, each time while the updater took a release sent through the Effector server (`issues-human/README.md` §19):
- 2026-11-26, boot 108, and 2026-11-27, boot 113: `[USB] STORAGE: WRITE REFUSED: SENSE KEY 3 ASC 03 ASCQ 00 (MEDIUM ERROR)`, then `[UPDATER] INSTALL: REFUSED: Write`. The install was refused whole and the machine went on with the release it had.
- The first time the same release, sent again two minutes later, was installed. The second time nothing was sent again.

So the machine's self-update, which the maintainer counts among the system's important properties, failed on one refused write. He asked that the cause be found before the drive is written again.

## What was found (2026-11-27)

**The drive.** A Kingston SNS4151S316G, a 16 GB mSATA SSD (firmware S9FM01.1), behind a JMicron JM20329 USB 2.0 to SATA bridge (`152D:2329`). Read from Ubuntu on the Ryzen, nothing written; the SMART values through the bridge's ATA pass-through:
- **The SSD shows no failing medium:** retired blocks 0, program and erase failures 0, reported uncorrectable 0, reallocation events 0; erase counts 64 on average and 151 at most. Power-on hours 489, power cycles 1675, unsafe shutdowns 9; its write cache is on.
- **The SSD's own error log holds no error.** It never ended a command with an error that it logged.
- **The SATA link between bridge and SSD:** 2 interface CRC errors (attribute 199) and 2 PHY errors (168) in the drive's life. When they happened is not known.
- **The whole device reads** without one kernel error line: 16 GB at about 37 MiB/s. SMART after it is the same.
- Linux's journal on that machine keeps only the day's boots, and none has an I/O error.

**The refusal.** Sense key 3 with ASC 03h ASCQ 00h is PERIPHERAL DEVICE WRITE FAULT. Since the SSD logged no failed command, it most likely came from the bridge: its own fault, or an error on the SATA link it reported this way. That is a reading from the evidence, not a cause shown.

**The difference from Linux, which is ours to mend.** Linux v7.0's SCSI layer (`scsi_check_sense`) takes a MEDIUM ERROR as worth trying again (`NEEDS_RETRY`) unless its ASC is 11h, 13h or 14h, and `sd` tries a command up to five times (`SD_MAX_RETRIES`). It also tries an ABORTED COMMAND again, and a device that is becoming ready. `usb_storage` gave up at the first refusal of every kind but a unit attention. Under Linux this drive's two refusals would most likely have been tried again and not been seen at all.

Linux's quirk for this bridge (`unusual_devs.h`: `US_FL_IGNORE_RESIDUE | US_FL_SANE_SENSE`) does not bear on it: `usb_storage` does not use the residue, and asks REQUEST SENSE for 18 bytes.

## Plan

1. **Retried as Linux does** (`usb_storage/src/recovery.rs`, `again`):
   - which refusals are tried again: MEDIUM ERROR but ASC 11h, 13h and 14h; ABORTED COMMAND; NOT READY with ASC 04h ASCQ 01h;
   - how many times: up to `RETRIES` = 5 more, as Linux;
   - the pauses, 0, 10, 20, 50 and 100 ms, are ours, not Linux's;
   - what is not tried again: hardware errors, illegal requests and protected media.

   The data of a WRITE is still in place for each try (211-DRV-0019).
2. **Each try logged** with the operation, its sectors and count, the last sense and the sector the device names (the sense's information field), and whether it was done or given up. At most 64 such lines a boot.
3. **Measured on the Ryzen:** releases sent to it as before, and every try again read from the log. Retries that come often would point at the bridge or the link, and the cheapest step then is another adapter, not another SSD.

## Acceptance criteria

- [x] Host cases (`tests/usb_storage_host.rs`): a write fault tried again five times with its pauses and then given up; the medium errors Linux takes as final not tried again; an aborted command and a device becoming ready tried again; a hardware error, an illegal request and a protected medium never. The write-fault case fails if medium errors are not tried again.
- [ ] The QEMU groups that use `usb_storage` pass as before.
- [ ] On the Ryzen: releases installed through the server, and every try again (if any) in the log with its sector.
- [ ] In `main`.

## Related

`issues-human/README.md` §19 (the drive), 211-DRV-0019 (a refused command explained, the data kept for a repeat), 000-DRV-0058 (a device that came back).
