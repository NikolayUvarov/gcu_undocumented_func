# Requests for the phones and mobile devices track (MOB), not numbered yet

**Owner:** `MOB` (the assessing session) · **Status:** open (one request, 2026-11-20) · **Recorded by:** the kernel session (`UPD`), for [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md)

The `MOB` track numbers its own tasks (`NNN-MOB-MMMM`), so requests from other tracks wait here. `MOB` turns each into a task and removes it from this file. The file is removed when it is empty.

## 1. A catalog of devices for `board_image.py` (for 352-UPD-0049)

**Why.** The maintainer (2026-11-20) asked for an image or a package that installs MIND Core on ARM devices such as the Orange Pi boards and phones. [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md) builds each device's image from one release. It needs one place that says what each device takes. Today the two boards' details live in `scripts/board_uboot.sh` and `scripts/board_image.py` ([100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md)), and the PinePhone's in [100000](100000-a-phone-the-pinephone-first.md).

**Asked:**
1. **One entry per board or phone,** in a file that `board_image.py --board <name>` reads:
   - the SoC;
   - the firmware: the script that fetches or builds it, its source and version, and its SHA-256;
   - where the firmware goes: on the card (U-Boot with TF-A at 8 KiB) or on the device's own flash (Tow-Boot on the PinePhone's SPI);
   - the boot partition's start, and the device tree U-Boot hands over;
   - where the firmware goes for a boot from the eMMC, for 352-UPD-0048's installer.
2. **`board_image.py` takes a boot volume given to it** (a FAT image from a release), not only a local build. `UPD` adds the check against the release's signed manifest, in 352-UPD-0049.
3. **The entries:** the Orange Pi Zero Plus 2 and Zero 3 first; the PinePhone when the phone is at hand.

**Not asked:** any change to U-Boot's or Tow-Boot's builds, or to the boards' runs of 100004-MOB-0002 and 0003.
