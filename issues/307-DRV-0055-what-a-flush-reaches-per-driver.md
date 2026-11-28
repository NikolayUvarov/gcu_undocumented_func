# 307-DRV-0055 — What a flush reaches, stated by each block driver

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Constitution:** MC-6.6, MC-12.1, MC-12.3

Numbered from STO's request of 2026-10-27 in `requests-DRV.md`, its second half. The first half is [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done).

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

- [x] **QEMU, for each driver:** a flush reaches the device, and a device without a cache says so (x86, 2026-11-28; see Progress).
- [ ] **The real machines' logs** name what their disks did: the MacBook Pro's USB stick, the Ryzen's NVMe, the DEXP's stick.

## Progress (2026-11-28)

**Built** on `claude/307-DRV-0055-flush-per-driver`. What each driver does with a client's flush (`block.wit` 2.0):

| Driver | Before | Now |
|---|---|---|
| `ahci` | FLUSH CACHE EXT, always | FLUSH CACHE EXT where IDENTIFY word 83 gives 48-bit addressing and that command (bits 10 and 13), FLUSH CACHE otherwise. No command where word 82 bit 5 (no volatile write cache) or word 85 bit 5 (it is off) is clear, those words valid by words 83 and 87: answered done |
| `ata` | FLUSH CACHE, always | as `ahci`, by the same code (`ahci/src/cache.rs`, which `ata` includes) |
| `nvme` | FLUSH when Identify's VWC (byte 525 bit 0) is set; without it answered done, nothing said | the same, and the instance that writes asks Get Features (Volatile Write Cache, 06h): a cache that is off gets no command either, answered done. A read-only instance asks nothing more and refuses every flush, as before (213-DRV-0060) |
| `usb_storage` | SYNCHRONIZE CACHE(10), always; a refusal logged with its sense (211-DRV-0019) | the same, unless MODE SENSE(6)'s caching page says WCE is clear: no command, answered done. MODE SENSE for all pages now asks 192 bytes, the length Linux and Windows ask of a USB disk (it asked 4), to see that page; WP is read only from a header that came (`usb_storage/src/mode.rs`). A device that gives no caching page gets the command |
| `virtio_blk` | `VIRTIO_BLK_T_FLUSH` when `VIRTIO_BLK_F_FLUSH` was negotiated; without it answered done, nothing said | the same. Without that feature the device writes through (virtio 1.1, 5.2.6.2) |
| `ramdisk` | the `Driver` trait's default: answered done | answered done: nothing to reach |

In every driver a flush the device refuses (an error status) or does not answer by its deadline is answered failed (`io`), never done, as before. New is the log: each driver says once of each kind what its flushes did, in a line that starts `<tag> FLUSH: `: sent and done; no write cache to empty (and which word said so); refused by the device (with its status, or for USB its sense in the line before); not answered. No IDL, ABI, `init` or `libmind` change.

**Tested on this machine (LEG9, WSL2), 2026-11-28.** Tests, not proofs (MC-12.2); QEMU's emulated devices only (MC-12.1).
- **Host:** `tests/flush_host.rs` (7 cases: IDENTIFY words valid and not, the caching page found, cut by the transfer or the mode data length, missing, after a subpage-format page), `tests/nvme_host.rs` (VWC and Get Features; a read-only instance never asks), `tests/ata_ahci_waits_host.rs` (the command the flush sends, a drive's error). All of `scripts/host_tests.sh`: exit 0, 283 result lines all ok (6860 tests), 20 min.
- **QEMU 8.2.2, x86, 4 CPUs, OVMF:** the `block` suite's new `flush_check` (`tests/qemu_smoke.py`), three boots, 42 s in all. The test's `vfs_server` boots from a disk no driver serves (IDE's secondary master) beside one disk on each bus: IDE's primary master (`ata`), AHCI, USB, VirtIO and NVMe, each with a blank partition it writes, flushes and reads back.
  - **Caches on:** QEMU's trace (turned on through QMP once the kernel runs, so not the firmware's commands) shows FLUSH CACHE EXT on both IDE buses, one FLUSH on the NVMe namespace, one SYNCHRONIZE CACHE(10) on the SCSI disk and one VirtIO flush; each driver said once it was sent and done.
  - **Caches off** (`cache.writeback=off`; for VirtIO `config-wce=off` too, as QEMU otherwise offers `VIRTIO_BLK_F_FLUSH` and turns the cache on for a driver that takes it): no flush in the trace on any device, which the trace's writes show it was recording; each driver said once it had no cache to empty, from IDENTIFY word 85, Get Features, the caching page's WCE and the missing feature; each flush answered done.
  - **Every flush failed by the host** (blkdebug, EINVAL; the USB disk answers it with ILLEGAL REQUEST, INVALID FIELD IN CDB, as a stick that refuses SYNCHRONIZE CACHE does): each flush answered failed, never done, each driver saying so once (AHCI's drive with ERROR 04, STATUS 41; NVMe's with status type 2, code 0x80; VirtIO's with status 1).
  - Each boot the images hold the sectors written. The block suite's earlier part (ATA, AHCI and USB boot disks) passed as before.
- **CI groups** (`scripts/ci_local.sh --only x86`, this tree, 4 CPUs): build (x86 test programs), 158 s; x86: services, storage, audio, 1172 s, 30 PASS; x86: files and block writes, 1210 s, 54 PASS (the flush check among them); x86: NVMe boot disk, 98 s (`nvme2`: the read-only `nvme#1` still sends nothing but Identify, the queues and reads, and refuses its write-probe build's flush); x86: USB image, 191 s (on the stick image `usb_storage` read WCE from the 192-byte MODE SENSE and said its first flush was done). Every group PASS, none skipped. `ARCH=aarch64 ./02_build.sh` builds.

Not shown here: what a real disk does (criterion 2), and aarch64 in QEMU, where the block suite does not run (`tests/block_app.rs` is x86 code) and no aarch64 group was run for this.

**Left for criterion 2:** a release with this on the MacBook Pro, the Ryzen and the DEXP, and the `FLUSH: ` line each disk's driver says at its first flush: whether the Mac's and the DEXP's sticks give a caching page and take SYNCHRONIZE CACHE, and whether the Ryzen's NVMe has its cache on. A stick that refuses the command answers every flush failed, as it did before this task.

**Seen while doing it, not changed:** `nvme` waits about 2 s for any command, a flush too, where `ahci` and `ata` give a flush 30 s. A real controller with much in its cache could take longer; its flush would then be answered failed, and the queue's next completion would be out of step.

**`block.wit` (plan point 4): a proposal, not made.** For the store's class a flush answered done means the same with or without a cache: the sectors are on the medium either way, a write-through device putting them there before it answers the write. So `flushed` stays right for a device with no cache, and the store needs no new answer for it. What the store cannot tell apart today is a device that refuses every flush (a USB stick that answers SYNCHRONIZE CACHE with ILLEGAL REQUEST; Linux sends such a stick none, taking it as write-through) from one that failed: both are `io`, so on such a stick every publication fails, as before this task. If STO wants such a medium usable with a weaker class (written, not flushed), one minor version of `block.wit` (2.1) could carry both: `flush` answering whether there was a cache to empty, and an error apart from `io` for a flush the device does not take. That is STO's to decide with DRV.

## Related

[307](307-the-stores-own-partition.md), [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done), [211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md).
