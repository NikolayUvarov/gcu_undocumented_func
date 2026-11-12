# 308-STO-0045 — A store's medium read without its key shows no content, names or CIDs, in QEMU

**Type:** tests (storage) · **Owner:** `STO` track · **Priority:** P1 · **Status:** in progress (2026-11-12) · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md) · **Roadmap:** track B "encryption and key management before confidential data" · **Constitution:** MC-4.8, MC-11.9, Appendix B.3

## Problem

308's second criterion asks, in QEMU on x86 and aarch64:
- the store on a sealed medium across a reboot;
- a stick read without the key, showing no content, names or CIDs.

**The first half is shown.** The `store` suite's disks keep their objects and names across reboots, sealed since 308-STO-0021, with the volume key kept by `keystore` (308-STO-0022).

**The second half is shown on the host only.** `tests/blockstore_host.rs` finds no block, key or CID of the store on its in-memory medium. No suite reads a medium the platform wrote, the way someone with the disk would, and looks for what the store put there.

## Plan

- **In the `store` suite's `store_partition_disk_check`** (307-STO-0044), on its NVMe and AHCI partitions on both architectures:
  - the name published is a long one, unlikely to occur by chance in sealed bytes;
  - after the machine is off, the host reads the store's partition as a disk taken elsewhere would be read: with no key.
- **None of these may be in it:**
  - 64 bytes of the object `blocks pattern` wrote;
  - the name;
  - the object's CID as text, as binary, and its digest.
- **A positive control:** the records' frames, which are in the clear by design (docs/storage), must be found there, so the bytes read are the store's.

## Acceptance criteria

- The check passes in the `store` suite on x86 and aarch64.
- 308's criteria are then all met, and 308 can be closed.

## Related

[308](308-encryption-at-rest.md), [308-STO-0021](../issues-done/308-STO-0021-sealed-records.done), [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done), [307-STO-0044](../issues-done/307-STO-0044-the-store-on-an-nvme-and-an-ahci-partition.done).
