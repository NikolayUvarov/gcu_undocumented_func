# 307-KRN-0177 — `init` finds the store's own partition and grants `blockstore` a client bounded to it

**Type:** `init` (media, grants) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** the drivers track's client bounded to a range ([requests-DRV.md](requests-DRV.md), "A block client bounded to a range", 2026-10-27) · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.1

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-27, "The store's own partition: media classified and a bounded grant (for 307)".

## Problem

- **Where the store's disk comes from.** Today `blockstore` gets a whole VirtIO disk that is blank (300-KRN-0025). That rule is safe only in QEMU.
- **The risk on a real machine.** A blank disk may be someone's new disk, and another system's disk is never written (the Ryzen's Windows disk, 213).
- **What 307 wants instead:** a partition of the store's own, and a client that reaches only its sectors.

## Plan

1. **`init` classifies the media of every block driver** (`nvme`, `ahci`, `usb_storage`, `virtio_blk`). It reads each disk's partition table through the driver's client and logs every partition it finds: number, type, label and size.
2. **It finds the store's partition:**
   - a partition labelled `MIND STORE` of the store's own type;
   - or the one the owner names in the configuration (`data/mode.conf` or a file of the store's; STO names it).
3. **`blockstore` gets a client bounded to that partition's sectors** (the drivers' part). `vfs_server` gets none of that partition.
4. **On a real machine no disk is given because it is blank.** The VirtIO rule of 300-KRN-0025 stays for QEMU only, where the disk is the suite's.
5. **Without such a partition** `blockstore` starts with no disk and says so, and `init` logs why (MC-12.1).

## Acceptance criteria

- **QEMU, x86 and aarch64:**
  - a disk with a `MIND STORE` partition beside a FAT one: `blockstore` writes only inside the store's sectors, and the FAT partition reads back unchanged;
  - a blank NVMe disk on x86 is not given to the store.
- **Host tests:** the partition table parser on GPT and MBR samples, including one with no store partition.
- **The `store` suite** passes as before on its VirtIO disk.

## Related

[307](307-the-stores-own-partition.md), 300-KRN-0025, [requests-DRV.md](requests-DRV.md), 213.
