# 307-KRN-0236 — The copy's partition on a real machine, named in `store.txt`

**Type:** kernel track (`init`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Roadmap:** track B · **Constitution:** MC-4.8, MC-2.3, MC-12.1

## Problem and plan

STO's request of 2026-11-12 (from `requests-KRN.md`), for 307's plan item "the copy's medium on real machines". The request as it was written, kept whole:

**Asked by** STO on 2026-11-12, for [307](307-the-stores-own-partition.md)'s plan item "the copy's medium on real machines".

- **Today:**
  - `blockstore` keeps its copy of chosen names on a second medium in slot 4 (309-STO-0040);
  - `init` lends one only in QEMU, the second VirtIO disk that is blank or a store's (309-KRN-0209);
  - a real machine has no such disk, and two candidates of the store's type and superblock are refused (307-KRN-0177).
- **Asked of `init`:**
  - **The word:** a line `copy <PARTUUID>` in `store.txt` names the copy's partition, as `partition <PARTUUID>` names the store's. That is STO's choice of word, yours to change.
  - **Not a second candidate:** the partition named as the copy, blank or with the store's superblock, does not count against the store's own.
  - **The client:** `init` gives `blockstore` a write client bounded to it in slot 4, as the store's own partition is given in slot 2.
    - It uses window 1 of its driver, or window 2 where that driver already holds the store's window: two partitions of one disk.
  - **The same disk:** when both partitions are on one disk, `init` says so in its log (`THE COPY SHARES A DISK WITH THE STORE: NOT AN INDEPENDENT COPY`, MC-4.8), and gives it all the same. 307's plan had the store say it, but only `init` knows the disks.
  - **After a restart** of the copy's driver, its window is set again only when the partition checks out the same, as the store's is (307-KRN-0177).
- **Checked by** a QEMU check of STO's, once this is in. It follows `store_partition_disk_check` (307-STO-0044):
  - the store on an NVMe partition and the copy on an AHCI partition, both named in `store.txt`;
  - the copy's names found after a reboot, and a record damaged on the store repaired from the copy;
  - one boot with both partitions on one disk, to show the log line.

## Acceptance criteria

- What the request's "Checked by" names, run by the asking track once this is in; `init`'s or the loader's own lines in the log.
- The tests the change touches pass on x86 and aarch64 in QEMU.

## Related

- [requests-KRN.md](requests-KRN.md), where it was asked.
