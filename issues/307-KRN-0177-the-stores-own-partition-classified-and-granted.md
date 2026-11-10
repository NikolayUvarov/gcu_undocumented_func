# 307-KRN-0177 — `init` finds the store's own partition and grants `blockstore` a client bounded to it

**Type:** `init` (media, grants) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (step 1 made on `claude/307-KRN-0177-store-partition`, 2026-11-10) · **Blocked by:** — (the client bounded to a range is [307-DRV-0054](307-DRV-0054-a-block-client-bounded-to-a-window.md)'s windows, made by STO and about to land; this branch is built on them) · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.1

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

## Progress (2026-11-10)

**Step 1, made: the tables and the rule, without `init` yet.** `libmind/src/partition.rs`, built on STO's branch at c3938027 (the windows):
- **The tables:** `read_table` reads a disk's MBR or GPT through any sector source.
  - GPT's header and entries are taken only when both CRCs match. A table without them gives no partition.
  - A partition outside the disk is left out and named.
  - A FAT volume over the whole disk, or a blank disk, has no table.
  - Up to 16 partitions are kept per disk; the rest are counted.
- **A partition** carries its number, type (MBR byte or GPT GUID), first sector, count, PARTUUID and GPT's name.
- **The rule:** `candidate` takes the store's type with the store's superblock both, or the partition `store.txt` names, which may be blank. No label is matched.
- **`store.txt`:** `named` reads its line `partition <PARTUUID>` in either form, and refuses anything else with a reason. `parse_id` and `id_text` read and write a PARTUUID as Linux writes it.

**Tested on the host:** `tests/partition_host.rs`, 11 tests, in `scripts/host_tests.sh`.
- **Samples:**
  - the stick's MBR (two FAT partitions and the store's);
  - the store's type without its superblock, which is taken only when named;
  - a superblock on another type;
  - a disk with no store partition;
  - a GPT disk with labels;
  - GPTs with a wrong header CRC, a wrong entries CRC or no header;
  - partitions past the end;
  - more than 16 partitions;
  - `store.txt` in both forms and seven malformed lines.
- **The test's own CRC** is checked against the standard's check value first.
- **Five faults put in by hand were each caught:**
  - either CRC check skipped;
  - the superblock not required;
  - a GUID's byte order;
  - an MBR bound.

**Left:**
- **Step 2:** `init` classifies the disks, sets the window and grants `blockstore` its client. It sets the windows again after a driver's restart.
- **Step 3:** the QEMU checks of the acceptance criteria, on x86 and aarch64.

## Related

[307](307-the-stores-own-partition.md), 300-KRN-0025, [requests-DRV.md](requests-DRV.md), 213.
