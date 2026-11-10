# 307 — A durable medium on real machines: the store's own partition (track B, eighth step)

**Type:** main task · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** [307-DRV-0055](307-DRV-0055-what-a-flush-reaches-per-driver.md) (what a flush reaches), [307-KRN-0229](307-KRN-0229-vfs-servers-clients-bounded-to-its-own-partitions.md) (`vfs_server` off the store's partition) · **Roadmap:** track B "durability classes" · **Constitution:** MC-4.4, MC-4.8, MC-12.1, preamble ("preservation of state"), Appendix B.3

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
- **The copy's medium on real machines** (moved here from [309](../issues-done/309-copies-scrub-and-repair.done), 2026-11-05).
  - `blockstore` holds its copy of chosen names on a second medium in slot 4 (309-STO-0040), which `init` lends in QEMU only (309-KRN-0209).
  - On a real machine it is a partition of another disk than the store's: two partitions of one disk are not independent copies (MC-4.8).
  - It is granted as the store's own partition is, and the store says when its copy shares a disk with it.
- **Durability classes (MC-4.4, B.3).** Each write says what it reached: `volatile` (a RAM disk), `flushed` (one device's flush), `copied` (two media, [309](../issues-done/309-copies-scrub-and-repair.done)). `publish`, `unpublish`, `commit` and `pin` reply with it, and `stat` gives the medium's; a `put` is retained only by its lease until a name or a pin retains its object (MC-4.11), so its class is the medium's ([307-STO-0019](../issues-done/307-STO-0019-how-far-a-write-reached.done)). This changes the interface: `blockstore.wit` 2.0, with 1.x refused. Before the first release a new layout or interface replaces the old one, which is refused by name; no conversion code (AGENTS.md section 3, the maintainer's decision of 2026-10-25).

### The partition's size (2026-11-09, 307-STO-0042)

A mount, a collection and a scrub read the whole medium.
- **Under TCG on x86** a mount after a reboot took 35 s at 1 GiB and 292 s at 4 GiB, with each pass reading 128 sectors per call to the driver ([307-STO-0042](../issues-done/307-STO-0042-a-pass-over-the-medium-reads-ahead.done)).
- **The index** reaches its most, 2^20 slots, at 4 GiB, and indexes 16 GiB of blocks at most.
- **So the stick's partition** starts at about 1 GiB until a real machine's times are measured and 310 brings the index onto the medium.

## Acceptance criteria

- The tasks: [307-STO-0019](../issues-done/307-STO-0019-how-far-a-write-reached.done) (durability classes), [307-STO-0020](../issues-done/307-STO-0020-the-stores-partition-on-the-stick.done) (the partition on the stick); `KRN`'s and `DRV`'s parts get their numbers from their requests.
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

## Progress (2026-11-10)

**Done:**
- [307-STO-0019](../issues-done/307-STO-0019-how-far-a-write-reached.done): the durability classes, `blockstore.wit` 2.0.
- [307-STO-0020](../issues-done/307-STO-0020-the-stores-partition-on-the-stick.done): the store's partition on the stick, with its superblock.
- [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done): a block client bounded to a window of sectors, in every driver's serve loop.
- [307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done): `init` finds the store's partition and gives `blockstore` window 1 of its driver.
- [307-STO-0042](../issues-done/307-STO-0042-a-pass-over-the-medium-reads-ahead.done): a pass over the medium reads ahead, for the partition's size.
- [307-STO-0044](../issues-done/307-STO-0044-the-store-on-an-nvme-and-an-ahci-partition.done): the store on a partition of an NVMe and of an AHCI disk, beside other systems' partitions.

**What QEMU has shown,** on x86 and aarch64 (`tests/usb_image_smoke.py`, 307-KRN-0177):
- the store mounts the stick's partition through window 1, its size the partition's, while `vfs_server` reads its volumes on the same drive;
- a blank NVMe disk is not given and not written;
- two candidates are refused and neither is written;
- a blank partition is taken only once `store.txt` names it, and the store then formats it through the window, as the host reads back.

**What the `store` suite shows** (307-STO-0044):
- the store on an NVMe partition (GPT) on x86 and aarch64, and on an AHCI partition (MBR) on x86;
- an object and a name found after a reboot;
- on the host, every sector outside the store's partition as written, another system's partitions before and after it and the disk beside it among them.

**Left for the second criterion:** the store on an AHCI partition on aarch64. Under AAVMF `ahci` finds no disk, which DRV has as [307-DRV-0068](307-DRV-0068-ahci-on-a-controller-the-firmware-left-untouched.md).

**Left besides:**
- [307-DRV-0055](307-DRV-0055-what-a-flush-reaches-per-driver.md): what a flush reaches, per driver;
- [307-KRN-0229](307-KRN-0229-vfs-servers-clients-bounded-to-its-own-partitions.md): `vfs_server` still holds the stick whole;
- [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md), step 5: a medium that comes back keeps its windows;
- the copy's medium on a real machine;
- the MacBook Pro's and the Ryzen's sticks, and the profile's Article 4 row.

## Related

[300](../issues-done/300-checksummed-block-store.done), 300-KRN-0025, [308](308-encryption-at-rest.md), [309](../issues-done/309-copies-scrub-and-repair.done), `requests-KRN.md`, `requests-DRV.md`.
