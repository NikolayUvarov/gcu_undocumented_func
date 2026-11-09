# 307-DRV-0055 — What a flush reaches, stated by each block driver

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Constitution:** MC-6.6, MC-12.1, MC-12.3

Numbered from STO's request of 2026-10-27 in `requests-DRV.md`, its second half. The first half is [307-DRV-0054](307-DRV-0054-a-block-client-bounded-to-a-window.md).

## Problem

The store reports `flushed` only when a device said its flush was done, and the profile says so per machine. So each driver must:
- send its device's flush when a client asks for one;
- say when the device has no write cache, or ignores a flush.

## Plan

1. **Each driver's flush, checked and stated:**

   | Driver | Command |
   |---|---|
   | `nvme` | FLUSH |
   | `ahci` and `ata` | FLUSH CACHE EXT, or FLUSH CACHE without 48-bit addressing |
   | `usb_storage` | SYNCHRONIZE CACHE(10); a refusal is already logged as "A FLUSH CANNOT EMPTY THE DEVICE'S CACHE" (211-DRV-0019) |
   | `virtio_blk` | `VIRTIO_BLK_T_FLUSH` when `VIRTIO_BLK_F_FLUSH` is offered |
   | `ramdisk` | nothing to reach |

2. **A device with no write cache** (IDENTIFY's volatile write cache bit, NVMe's VWC, MODE SENSE's caching page where the device answers it) answers its flush as done and says once in the log that it has no cache to empty.
3. **A device that refuses or ignores a flush** answers the flush with an error, never as done, and says so once in the log.
4. **`block.wit`:** whether the answer carries "no cache" apart from "done" is settled with STO when 307-DRV-0054's version is set, so that one minor version carries both.

## Acceptance criteria

- **QEMU, for each driver:** a flush reaches the device, and a device without a cache says so.
- **The real machines' logs** name what their disks did: the MacBook Pro's USB stick, the Ryzen's NVMe, the DEXP's stick.

## Related

[307](307-the-stores-own-partition.md), [307-DRV-0054](307-DRV-0054-a-block-client-bounded-to-a-window.md), [211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md).
