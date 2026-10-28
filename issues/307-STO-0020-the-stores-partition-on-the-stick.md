# 307-STO-0020 — The store's partition on the stick

**Type:** image (storage) · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Constitution:** MC-4.4, MC-12.1

## Problem

The stick `make_usb_image.py` writes has the boot partition and `MIND LOG`, and nowhere for the store. A blank disk must never be taken for it on a real machine ([307](307-the-stores-own-partition.md)), so the store needs a partition it can be known by.

## Plan

- **A third MBR entry after `MIND LOG`:** type `0x7F` (reserved for local and experimental use), 128 MiB, aligned to 1 MiB.
- **Formatted as an empty store:** the store's superblock first (its magic, layout and sector size, and their SHA-256), the rest blank. A partition is the store's by that superblock, not by being blank. The script reads the layout from `blockstore/src/store.rs`, so the two cannot disagree.
- **`check_image` checks it:** its type, its place after `MIND LOG`, its size and its superblock. It refuses a fourth entry.
- **Nothing uses it yet.** `init` giving `blockstore` a client bounded to it is `KRN`'s and `DRV`'s part (the requests for 307). `vfs_server` mounts only FAT types, so it leaves the partition alone.

## Acceptance criteria

- A host test: a medium holding only that superblock mounts as an empty store with nothing written, and a blank medium the store formats holds the same bytes.
- An image from `make_usb_image.py` passes `check_image` with the partition, and still boots (`tests/usb_image_smoke.py`, x86 and aarch64).
- docs/write-disk.md and write-disk_RU.md list the partition.

## Related

[307](307-the-stores-own-partition.md), [211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md) (`MIND LOG`), `requests-KRN.md`, `requests-DRV.md`.
