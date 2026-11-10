# 307-KRN-0177 — `init` finds the store's own partition and grants `blockstore` a client bounded to it

**Type:** `init` (media, grants) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (made and tested on `claude/307-KRN-0177-store-partition`, 2026-11-10; landing left) · **Blocked by:** — (the client bounded to a range is [307-DRV-0054](307-DRV-0054-a-block-client-bounded-to-a-window.md)'s windows, made by STO and about to land; this branch is built on them) · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.1

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

## Progress (2026-11-10, steps 2 and 3)

### Step 2, made: `init` and the drivers' loop

**One buffer per client badge** (`libmind/src/block_protocol.rs` `Buffers`, `block::serve`), found while making step 2.
- **The flaw before:** `serve` kept one transfer buffer per driver instance, and each `attach` replaced it. So two clients of one disk would read into each other's buffer.
  - The stick's case is exactly that: `vfs_server`'s FAT partitions and `blockstore`'s window on one `usb_storage`.
  - So is `init` reading a disk's table while `vfs_server` uses it.
- **Now:** each badge attaches its own, up to 8.
  - A badge attaching again replaces only its own.
  - With every place taken, `attach` is refused (`limit`).
- **Tested:** `tests/block_host.rs` checks that reads land in each badge's own buffer.

**`init`**, when no VirtIO disk is the store's (300-KRN-0025):
- **The tables.** It reads and logs the table of every `nvme`, `ahci`, `usb_storage` and `virtio_blk` disk, once: the scheme, each partition's number, type, sectors, PARTUUID and GPT name.
- **The choice:**
  - it takes the one partition of the store's type with its superblock, or the one `store.txt` names;
  - it says why it took it;
  - with two candidates or more it takes none, and names each.
- **The grant.** It sets window 1 on that driver, through a client badged `BADGE_WINDOWS` that it drops at once. It gives `blockstore` the window's write client, `badge_for(1, true)`, in slot 2.
- **After the driver starts again** it reads the table again. It sets the window again only when the partition has the same number, PARTUUID and sectors and is still a candidate. Never by the number alone.
- **The copy's disk** (slot 4) stays a VirtIO disk's, given only beside a VirtIO disk of the store's.
- **`ata` is not read:** the plan's list is `nvme`, `ahci`, `usb_storage` and `virtio_blk`.

### Step 3, tested: `tests/usb_image_smoke.py`

Run on acc-1 through Effector, x86, at bfd38a3e: the group "x86: USB image" passed, 98 s.

**The main boot** of the real stick, which most suites do not boot, since they use `fat:` disks without partition 3:
- **`init`'s lines:**
  - `usb_storage: MBR, 3 PARTITIONS`, with each partition;
  - `THE BLOCK STORE'S PARTITION: usb_storage PARTITION 3 (ITS TYPE AND SUPERBLOCK), 262144 SECTORS FROM 1163264, WINDOW 1`.
- **`usb_storage`'s line:** `[BLOCK] WINDOW 1 SET`.
- **The store:** `READY ... SECTORS=1/262144 ... DURABILITY=FLUSHED`.
- **The rest of the boot,** with the store and `vfs_server` on the one drive:
  - `vfs_server`'s boot volume, `log:` and its files;
  - the unplug and replug;
  - another medium in the stick's place, left untouched.
- **On the host afterwards:**
  - the stick's table is unchanged;
  - the store's superblock holds its key, written in place;
  - the boot volume and `log:` pass `fsck.fat`.

**Four more boots, on fresh copies of the stick:**
1. **A blank NVMe disk beside it:** `nvme: NO PARTITION TABLE`. The stick's partition is taken, and the NVMe disk stays all zeros.
2. **An NVMe disk with a partition of the store's type and superblock:** `THE STORE'S PARTITION REFUSED: 2 CANDIDATES, NONE TAKEN`, and both are named. The store runs on `ramdisk#1` (`SECTORS=1/16384`), and neither candidate is written.
3. **The stick's partition 3 made blank:** it is logged as the store's type without its superblock and not taken. The store runs on `ramdisk#1`, and the partition stays blank.
4. **The same, with `store.txt` at the boot volume's root** naming `partition <signature>-03`: taken (`NAMED IN STORE.TXT`), and the store formats it (`MIND-STO` on the host afterwards).

### What this does not show (MC-12.3)

- **A medium that comes back.** A device that comes back keeps the driver's windows, since its driver did not restart (`usb_storage`, 000-DRV-0058).
  - So window 1 then reaches whatever medium is in the stick's place.
  - In this suite the store wrote nothing then, so the copy stayed untouched, but nothing stopped it.
  - Clearing the windows on a return, and `init`'s checked re-set, are added to [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md)'s plan.
  - A byte-identical copy of the stick would pass any check of its contents.
- **`vfs_server` still holds the stick whole** for its FAT partitions, so the store's sectors are inside its reach. That is [307-KRN-0229](307-KRN-0229-vfs-servers-clients-bounded-to-its-own-partitions.md), split from plan step 3.

### aarch64 and the other suites, at bfd38a3e

- **`tests/usb_image_smoke.py --arch aarch64`** on acc-1: passed, with the same lines and the same four boots. The store's key there is kept by `keystore`.
- **The groups the change touches,** through Effector:
  - on acc-2: "x86: files and block writes" (965 s), "x86: services, storage, audio" (678 s) and "x86: NVMe boot disk" (93 s);
  - on PCU: "aarch64: files and the block store" (947 s) and "aarch64: NVMe boot disk" (205 s).

  All passed. Their suites boot `fat:` disks without partition 3, so the store stays on its VirtIO disk or on `ramdisk#1` as before (criterion 3).
- **The host tests** in the kernel session's container all passed, the 11 of `tests/partition_host.rs` and `tests/block_host.rs` among them. `tests/manifest_test.py` was not run: it fails in this container's Python on `main` too.

**The acceptance criteria are met in QEMU on x86 and aarch64.** The issue closes when it lands. Its remainders are 307-KRN-0229 and 000-KRN-0085's step 5.

## Related

[307](307-the-stores-own-partition.md), 300-KRN-0025, [requests-DRV.md](requests-DRV.md), 213.
