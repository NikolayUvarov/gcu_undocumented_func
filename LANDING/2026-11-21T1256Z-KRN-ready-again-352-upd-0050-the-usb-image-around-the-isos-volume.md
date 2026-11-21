# KRN: ready again — 352-UPD-0050, the USB image around the ISO's boot volume

**Ready:** `claude/352-UPD-0050-release-artifacts` ecf6425b12

**For round 165.** It replaces KRN's 0900Z request for this branch (8586e2f53e), which round 164 neither landed nor returned.

**What it adds** to that request (ecf6425b12, plan steps 2 and 4 for the ISO and the USB image):
- **`make_usb_image.py`'s `pack_volume`:** the USB image around the FAT image `make_iso.boot_volume` made. Its MBR has the ESP's entry at 1 MiB, then the log and the store's partitions; `MIND LOG`'s dates are the build's, and the disk's signature comes from the volume's hash.
- **`release.py artifacts`:** one boot volume for each architecture, put into both the ISO and the USB image. QEMU's `vvfat` and `qemu-img` are not used there.
- **Docs:** docs/update/publishing.md 0.10 (EN and RU), and the issue's progress.

**Tested** in this session's container:
- `tests/release_test.py` 18 of 18. The artifacts test now makes the USB image too: its ESP is the ISO's FAT image byte for byte, and two runs give the same `SHA256SUMS`;
- a real run on an x86 build signed as version 6, twice: the same ISO, USB image, bundle and `RELEASE.json`, and `verify-sums` VERIFIED;
- that USB image booted in QEMU (OVMF, as a USB disk, read-only) to the shell, slot A confirmed, and its SHA-256 was the same afterwards.

`make_usb_image.py pack` (vvfat), which the tests and the writers use, is unchanged.
