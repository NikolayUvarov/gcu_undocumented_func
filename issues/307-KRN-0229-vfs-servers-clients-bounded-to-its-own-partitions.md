# 307-KRN-0229 — `vfs_server`'s clients of the store's disk bounded to its own partitions

**Type:** `init` (grants), `vfs_server` (mounts) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (made on a branch, ABI 32, 2026-11-10; QEMU runs left) · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) (STO's) · **Constitution:** MC-3.1, MC-5.1, MC-12.3

Split from [307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done), plan step 3's second half: "`vfs_server` gets none of that partition".

## Problem

Since 307-KRN-0177:
- `blockstore` reaches its own partition through window 1 of the disk's driver, and nothing else.
- `vfs_server` still gets that disk whole, write-badged, for its FAT partitions on the same stick (the boot volume and `log:`).
- So the store's sectors are inside `vfs_server`'s reach. Only its own code keeps it out of them, not its capability (MC-3.1).

## Plan

1. **Windows for `vfs_server`.** On the disk that holds the store's partition, `init` sets a window for each other partition `vfs_server` mounts (the FAT types it reads, at most 6 beside the store's) and gives `vfs_server` those windows' clients in place of the whole disk.
2. **`vfs_server` mounts a window as a volume over the whole device.** It already does that for a FAT volume without a table (`fat_starts`).
   - What it reads from the table now (the boot volume's partition number, the disk signature, `log:` as the partition after it, the update zone) comes from `init` with each client instead.
3. **The windows are set again after a restart,** and dropped after a return (000-KRN-0085), as the store's are.

## Acceptance criteria

- **QEMU, `tests/usb_image_smoke.py`, x86 and aarch64:**
  - `vfs_server` holds no client of the stick's whole disk;
  - the boot volume, `log:` and the update zone work as before;
  - a write by `vfs_server` past a window's end is refused.
- **`docs/profile/tcb.md`'s row for disk integrity** names `vfs_server` as outside the store's sectors.

## Progress (2026-11-10)

**Found: the plan's order cannot hold.**
- **What `init` knows when.** `init` reads `store.txt` through `vfs_server` (`read_boot_file`). So it knows the store's partition only after `vfs_server` holds its disks.
- **So plan step 1 cannot be kept as written.** "The disk that holds the store's partition" is not known when `vfs_server` gets its clients.

**What is made instead** (branch `claude/307-KRN-0229-vfs-server-gets-windows`, on 308-KRN-0205's, ABI 32):
- **Every disk with a partition table is given as windows, the store's or not.** `init` reads the table through a client of its own (`vfs_windows`). It sets a window for each partition `vfs_server` may mount, from window 2 of the disk's driver up:
  - an MBR entry of a FAT type (`partition::MBR_FAT`, the types `fat_starts` reads);
  - the GPT partition at the start the firmware booted from.
- **The slots.** `vfs_server` gets the windows' write-badged clients in slots 16 to 21 (`SLOT_VFS_PARTS_FIRST`, `VFS_PARTS` 6; ABI 32).
- **Its arguments say where each window lies:** `parts`, then, for each disk, its MBR signature in hex (0 on GPT) and its windows' first sectors, `S:F,F`.
- **What stays whole.** A disk without a table (blank, or one FAT volume over the whole disk), or one that did not answer `init`, is still given whole, in slots 2 to 4.
- **What the store may not take.** The store takes no partition of a disk `vfs_server` holds whole, and none whose sectors one of `vfs_server`'s windows covers; `vfs_windows` sets no window over the store's partition.
- **`vfs_server` sees such a disk through its windows, at their places** (`disk::Io::Windows`).
  - **One disk, as before.** The boot volume and `log:` keep one cache and one medium identity (000-KRN-0079): a stick put back with another boot manifest closes `log:` too.
  - **Volumes start where they lie on the disk:** the `AT LBA` of its lines, the boot volume's start and MBR signature as the firmware names them, `log:` as another window of the same disk.
  - **Routing.** A sector goes to the window that starts last at or before it. So a sector past that window's end reaches the driver, which refuses it; one before every window is refused by `vfs_server` itself.
  - **What else read the table: nothing.** The update zone and the boot records are files on the boot volume.
- **What the driver refuses is dropped** (`disk.rs`, `write_dirty`).
  - **The rule.** Changed sectors past the driver's reach (a window's end, or the device's) can never be written. They are dropped, named in the log (`[VFS] THE … DRIVE REFUSED n SECTORS FROM LBA x: PAST ITS REACH, NOT WRITTEN`), and one flush fails for them.
  - **Why.** Kept dirty, they would make every later flush of the disk fail, the boot volume's too.
- **After a driver starts again,** `init` sets each of `vfs_server`'s windows on it again while the partition is still there with the same number, PARTUUID and sectors (`vfs_windows_again`), as it does the store's. A returned medium (000-KRN-0085) ends its driver, so the same check covers it.

**Tested so far:**
- host tests: `partition_host.rs` (12, the FAT types and the overlap among them), `runtime.rs` (46), `boot_abi_host.rs` (9), `block_host.rs` (8);
- `init` and `vfs_server` built with no warning.

**The QEMU checks** (`tests/usb_image_smoke.py`):
- **On the stick:** `vfs_server` holds the boot and log partitions as windows 2 and 3, in its slots 16 and 17 (badges 5 and 7), and no client of the whole disk.
- **A `log:` volume whose boot sector claims 256 sectors past its partition**, with every free cluster inside it marked bad:
  - `vfs_server` mounts it at the size it claims;
  - what it writes past the partition (the boot's log, then `write log:past.txt`) is refused at its window's end and named in its log;
  - the write says so;
  - nothing of it reaches the sectors after the partition, the store's.
- **The block suite** (`block_suite`, `tests/block_app.rs`): the raw image gets a blank FAT32 partition after its file system, to the disk's end. Through ATA, AHCI and USB in turn, the test's `vfs_server`:
  - holds the two partitions as windows 2 and 3 (slots 16 and 17) and no client of the whole disk;
  - leaves the file system's window alone, and writes, flushes and reads back 8 sectors at the blank one's end;
  - is refused a sector past each window's end. On the host the first sector past the file system's window is still blank.

**Runs (2026-11-10, this session's container, 4 CPUs, QEMU 8.2.2 under TCG):**
- **x86, all passed:**
  - "x86: USB image", with both checks above;
  - "x86: boot, display, shell, memory, clock";
  - "x86: services, storage, audio";
  - "x86: NVMe boot disk".
- **aarch64, all passed:**
  - `usb_image_smoke.py --arch aarch64`, with both checks;
  - "aarch64: boot and fault containment";
  - "aarch64: files and the block store";
  - "aarch64: NVMe boot disk".
- **"x86: files and block writes" failed first, as it should have.** The old block suite looked for a whole disk in slots 2 to 4, and its partitioned disk now comes as windows.
  - **The fix.** The suite is changed as above.
  - **What the changed suite found.** Under AHCI, `ata` runs without a device and its client still comes whole (`NO DEVICE`), so the suite leaves such lines out.
  - **The group then passed** (1055 s): the block suite through ATA, AHCI and USB, and every other suite of the group.
- **A fault put in by hand, never committed:** `vfs_windows` set each window 256 sectors longer than its partition. The past-the-window boot then wrote past the partition (`WROTE`, `sync` `OK`), and the check failed. Without the fault it passes.

## Related

[307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done), [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done), [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md).
