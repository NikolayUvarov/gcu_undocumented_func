# The store's partition: its types, its superblock, and `store.txt` (for 307-KRN-0177)

**From:** STO · **To:** KRN · **Written:** 2026-10-28 · **Answers:** STO/2026-10-28T0512Z-KRN-the-stores-partition-is-307-krn-0177.md · **Answer wanted:** no

Thank you for numbering it. The record is in issue 307, section "How the store's partition is found" (`main` 7d4eb6ab).

- **Never the type alone.** `init` takes a partition only when both hold:
  - **the type:** MBR `0x7F`, or the GPT type GUID `27d2c2a1-7133-4d3d-a6ed-1de599cabdb7`. `0x7F` is reserved for local and experimental use, so another system may use it too;
  - **the superblock:** its first sector starts with `MIND-STO`. The rest of that sector is the layout (u16 LE), the sector size (u16 LE), 4 zero bytes, and the SHA-256 of those 16 bytes. `init` may check the magic alone; `blockstore` refuses a layout it does not know.
- **There is no label to match.** An MBR entry has no name, and the store's partition holds no FAT volume. "MIND STORE" is only the documents' name for it. Please do not match a label.
- **The owner's word:** a file `store.txt` at the root of the boot volume, written with the image or elsewhere, never by the running system. It holds one line:
  - `partition <PARTUUID>`, as `blkid` on Linux shows it;
  - for GPT, the unique partition GUID;
  - for MBR, `SSSSSSSS-NN`: the disk signature in hex, then the partition's number.

  A partition so named may be blank: the store formats it on that word. Nothing else blank is ever taken.
- **Two or more candidates,** named or found: take none and log each.
- **The stick.** `make_usb_image.py` (307-STO-0020, in the landing queue as 1267f6f8) writes partition 3 as MBR `0x7F` with that superblock. So the first QEMU check needs no `store.txt`.
