# 352-DRV-0118 — The drive's model as it names itself, kept by each block driver for `block.wit` 2.1

**Type:** drivers (`ata`, `ahci`, `nvme`, `usb_storage`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** open (2026-11-28) · **Blocked by:** `KRN`'s `block.wit` 2.1 (the call, and `mind::block::serve`'s `Driver` method; KRN's request of 2026-11-21 in [requests-DRV.md](requests-DRV.md), answered on 2026-11-28) · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-12.3, MC-12.4

## Problem

The installer's `disks` (`idl/install.wit` 1.0, [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done)) carries each disk's `model`. That way a person can tell two drives of one size on one bus apart. `block.wit` 2.0 carries no identity, so `model` is empty for every disk.

What the drivers do today:
- **`ata`** and **`ahci`** read IDENTIFY DEVICE at probe, keep the sector count, and drop the rest.
- **`nvme`** reads Identify Controller's MN into a local buffer, logs it (`CONTROLLER: … MODEL …`), and drops it.
- **`usb_storage`** sends no INQUIRY at all. It names a device by its USB vendor and product IDs only.
- **`virtio_blk`** has no model.

## Plan

KRN's split, as proposed on 2026-11-21:
- **KRN** adds the call to `block.wit` 2.1, the drive's model as it names itself, at most 40 bytes. `mind::block::serve` answers it from a `Driver` method that gives none by default.
- **DRV** fills that method. Each driver keeps its model from its probe and gives it as printable ASCII, with trailing spaces and NULs cut:
  - **`ata`** and **`ahci`:** IDENTIFY words 27 to 46, each word's two bytes swapped as ATA stores them.
  - **`nvme`:** Identify Controller bytes 24 to 63 (MN), already read for the log.
  - **`usb_storage`:** a standard INQUIRY (36 bytes) at attach, vendor bytes 8 to 15 and product bytes 16 to 31, joined by one space. A refused INQUIRY leaves the model empty and the device in use, and is said once.
  - **`virtio_blk`:** empty, unless the device offers `VIRTIO_BLK_F_ID` (a later step).

## Acceptance criteria

- [ ] Host cases for each driver's decoding:
  - ATA's swapped words;
  - NVMe's MN padded with spaces and with NULs;
  - INQUIRY's vendor and product;
  - a refused INQUIRY.
- [ ] QEMU: `install`'s disk list shows the models QEMU's emulated drives give: an IDE and an AHCI disk, an NVMe disk, and a USB stick.
- [ ] On a machine with two drives of one size (MC-12.1): the models tell them apart.

## Related

- [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done) (the installer service), [requests-DRV.md](requests-DRV.md) (the request).
