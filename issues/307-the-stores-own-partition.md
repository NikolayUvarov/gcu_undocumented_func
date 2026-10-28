# 307 — A durable medium on real machines: the store's own partition (track B, eighth step)

**Type:** main task · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md) (`init`'s grant, which waits for `DRV`'s bounded client) · **Roadmap:** track B "durability classes" · **Constitution:** MC-4.4, MC-4.8, MC-12.1, preamble ("preservation of state"), Appendix B.3

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

On a real machine the store lives on a RAM disk. `init` gives `blockstore` a disk only if it is a VirtIO disk that is blank or holds the store (300-KRN-0025); otherwise it gets `ramdisk#1`. So on the MacBook Pro or the Ryzen nothing the store holds survives a reset. For the substrate of a Mind the store is its memory, and that memory must outlive reboots on the machines it runs on.

## Plan

- **The medium.**
  - A partition of our own type, labelled `MIND STORE`, on the stick `make_usb_image.py` writes, beside `MIND LOG`.
  - On an internal disk, only a partition the owner names in the configuration.
  - **Never a disk because it is blank** on a real machine (QEMU's rule today): a blank disk may be someone's new disk, and another system's disk is never written (the Ryzen's Windows disk, 213).
- **A client bounded to the partition.**
  - `init` classifies the media of every block driver (`nvme`, `ahci`, `usb_storage`, `virtio_blk`).
  - `blockstore` gets a client bounded to its partition's sectors, and `vfs_server` gets none.
  - These are requests to `KRN` (the grants) and `DRV` (a client bounded to a range), not this track's code.
- **What a flush reaches, per driver:** NVMe FLUSH, ATA FLUSH CACHE EXT, SCSI SYNCHRONIZE CACHE. A device that reports no write cache, or ignores a flush, is named as such (MC-12.1). This is `DRV`'s to state.
- **Durability classes (MC-4.4, B.3).** `put` and `publish` say what they reached: `volatile` (a RAM disk), `flushed` (one device's flush), `copied` (two media, [309](309-copies-scrub-and-repair.md)). This changes the interface: `blockstore.wit` 2.0, with 1.x refused. Before the first release a new layout or interface replaces the old one, which is refused by name; no conversion code (AGENTS.md section 3, the maintainer's decision of 2026-10-25).

## Acceptance criteria

- QEMU, x86 and aarch64: the store on a partition of an NVMe, an AHCI and a USB disk; an object found after a reboot; a disk that is blank or another system's never written.
- The MacBook Pro's and the Ryzen's sticks: an object found after a reboot. A power cut done by the maintainer, its configuration named.
- `put` and `publish` report the class they reached; the profile's Article 4 row says what each machine reaches.

## How the store's partition is found (decided 2026-10-28)

The storage track's answer to 307-KRN-0177 (the kernel session asked for the type and the configuration's word).

- **Never by a type alone.** A partition is the store's only when both hold:
  - its type: MBR `0x7F` (reserved for local and experimental use, so another system may use it too), or the GPT type GUID `27d2c2a1-7133-4d3d-a6ed-1de599cabdb7` ("MIND Core block store");
  - its first sector is the store's superblock: `MIND-STO`, then the layout, the sector size and the SHA-256 of those 16 bytes (docs/storage; `make_usb_image.py` writes it, 307-STO-0020).
- **There is no label.** An MBR entry has no name. `MIND STORE` is only the name the documents give it.
- **The owner's word is the file `store.txt`** at the root of the boot volume. It is written with the image or on another machine, never by the running system, as `update.txt` and `netpolicy.txt` are. It holds one line:
  - `partition <PARTUUID>`, the partition's unique id as `blkid` on Linux shows it;
  - for GPT, its unique partition GUID;
  - for MBR, `SSSSSSSS-NN`: the disk signature and the partition's number.

  A partition the owner names may be blank: the store formats it, by that word. Any other partition is taken only with the type and the superblock. A blank disk or partition is never taken.
- **Two or more candidates:** `init` takes none and logs each, rather than choose.

## Related

[300](../issues-done/300-checksummed-block-store.done), 300-KRN-0025, [308](308-encryption-at-rest.md), [309](309-copies-scrub-and-repair.md), `requests-KRN.md`, `requests-DRV.md`.
