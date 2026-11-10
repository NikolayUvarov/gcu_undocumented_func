# 307-STO-0044 — The store on a partition of an NVMe and of an AHCI disk, its object found after a reboot, the other partitions untouched

**Type:** tests (storage) · **Owner:** `STO` track · **Priority:** P1 · **Status:** in progress (2026-11-10) · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Roadmap:** track B "durability classes" · **Constitution:** MC-4.4, MC-4.8, MC-12.1

## Problem

307's second criterion asks, in QEMU on x86 and aarch64, for three things:
- the store on a partition of an NVMe, an AHCI and a USB disk;
- an object found after a reboot;
- a disk that is blank or another system's never written.

307-KRN-0177's USB image suite covers the stick:
- the store mounts its partition through window 1;
- a blank NVMe disk is not written.

**No suite yet puts the store on a partition of an NVMe or an AHCI disk, or finds an object on a partition after a reboot.** No suite checks either that the partitions beside the store's stay as they were. Only the window keeps the store inside its own, and a fault in its offset or its clipping would write into another system's data.

## Plan

A check in the `store` suite (`tests/qemu_smoke.py`), on both architectures, in two machines. Each is booted twice, and each has two extra disks with images written in place (`snapshot=off`).

1. **The store on NVMe, with GPT:**
   - partition 1 is another system's (Microsoft basic data), partition 2 the store's (its type GUID and its superblock), partition 3 another system's (Linux data);
   - beside it, an AHCI disk that is wholly another system's: MBR, one partition of type `0x07`.
2. **The store on AHCI, with MBR:** the same three partitions as types `0x07`, `0x7F` and `0x83`, and beside it an NVMe disk that is wholly another system's (GPT, one partition).
3. **In each machine:**
   - `init` logs both tables and takes the store's partition on its driver, window 1;
   - the driver logs the window it set;
   - `blockstore` reports the partition's sectors and `FLUSHED`;
   - an object is put and a name published, both found again after a reboot.
4. **On the host afterwards:**
   - every sector outside the store's partition, on both disks, is byte for byte as written: the tables, GPT's backup and the other partitions;
   - inside the partition the store has written past its superblock, so the comparison is not vacuous.

## Acceptance criteria

- The check passes in the `store` suite on x86 and aarch64.
- A fault put in by hand into the window's offset is caught.
- 307's progress records what this shows and what it does not: a real machine's disks (MC-12.1).

## Related

[307](307-the-stores-own-partition.md), [307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done), [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done).
