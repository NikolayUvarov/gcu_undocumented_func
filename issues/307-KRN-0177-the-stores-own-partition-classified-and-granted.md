# 307-KRN-0177 — `init` finds the store's own partition and grants `blockstore` a client bounded to it

**Type:** `init` (media, grants) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** the drivers track's client bounded to a range ([requests-DRV.md](requests-DRV.md), "A block client bounded to a range", 2026-10-27) · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.1

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-27, "The store's own partition: media classified and a bounded grant (for 307)".

## Problem

- **Where the store's disk comes from.** Today `blockstore` gets a whole VirtIO disk that is blank (300-KRN-0025). That rule is safe only in QEMU.
- **The risk on a real machine.** A blank disk may be someone's new disk, and another system's disk is never written (the Ryzen's Windows disk, 213).
- **What 307 wants instead:** a partition of the store's own, and a client that reaches only its sectors.

## Plan

1. **`init` classifies the media of every block driver** (`nvme`, `ahci`, `usb_storage`, `virtio_blk`). It reads each disk's partition table through the driver's client and logs every partition it finds: number, type, label and size.
2. **It finds the store's partition** (STO's answer of 2026-10-28, issue 307, "How the store's partition is found"):
   - **Both of these, never the type alone:**
     - the type is MBR `0x7F` or the GPT type GUID `27d2c2a1-7133-4d3d-a6ed-1de599cabdb7`;
     - its first sector starts with `MIND-STO`.

     No label is matched.
   - **Or the owner's word:** a file `store.txt` at the boot volume's root, `partition <PARTUUID>` (GPT: the unique partition GUID; MBR: `SSSSSSSS-NN`, the disk signature and the partition's number). A partition so named may be blank: the store formats it.
   - **Two or more candidates,** found or named: none is taken, and each is logged.
3. **`blockstore` gets a client bounded to that partition's sectors** (the drivers' part). `vfs_server` gets none of that partition.
4. **On a real machine no disk is given because it is blank.** The VirtIO rule of 300-KRN-0025 stays for QEMU only, where the disk is the suite's.
5. **Without such a partition** `blockstore` starts with no disk and says so, and `init` logs why (MC-12.1).

## Acceptance criteria

- **QEMU, x86 and aarch64:**
  - the stick `make_usb_image.py` writes (partition 3, MBR `0x7F`, with the superblock): `blockstore` writes only inside the store's sectors, and the FAT partitions read back unchanged;
  - a partition named by `store.txt` is taken even when blank; two candidates are refused, each logged;
  - a blank NVMe disk on x86 is not given to the store.
- **Host tests:** the partition table parser on GPT and MBR samples, including one with no store partition, one of type `0x7F` without the superblock, and `store.txt`'s line in both forms.
- **The `store` suite** passes as before on its VirtIO disk.

## Related

[307](307-the-stores-own-partition.md), 300-KRN-0025, [requests-DRV.md](requests-DRV.md), 213.
