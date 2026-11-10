# 100004-MOB-0001 — The boards' card: U-Boot with TF-A before MIND Core's image

**Type:** image, script · **Owner:** `MOB` · **Priority:** P1 · **Status:** in progress (2026-11-10; the image's partition table corrected the same day: MBR, not GPT) · **Blocked by:** — · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-1.3, MC-12.1, MC-12.3

## Problem

**What an Allwinner board boots.** Its boot ROM reads a first-stage loader (U-Boot's SPL) from fixed sectors of the microSD card. The SPL sets up the DRAM, then starts TF-A's BL31 and U-Boot. U-Boot's UEFI then starts `EFI/BOOT/BOOTAA64.EFI` from the card's ESP.

**What MIND Core's image has.** `make_usb_image.py` writes an MBR disk: the boot partition (FAT16, type 0xEF) with the loader and the slots, then `MIND LOG` and the store's partition, each at 1 MiB alignment. Nothing in it is for a board's boot ROM.

**Where the two meet.** The ROM reads the SPL from 8 KiB in (sector 16), and `u-boot-sunxi-with-spl.bin`, about 0.5 to 1 MiB with BL31, follows it. The boot partition starts where QEMU's `vvfat` puts it: sector 63, 31.5 KiB in (read from a built image's MBR, 2026-11-10: the boot partition at 63, `MIND LOG` at 1032192, the store at 1163264). That is inside U-Boot. An MBR has no entries in the way, so only the boot partition has to move. The ROM's second place at 128 KiB is read in U-Boot's documentation, not needed for an MBR.

**The two boards' U-Boot:**
- **The H5:** `orangepi_zero_plus2` is in Debian's `u-boot-sunxi` (trixie, 2025.01) and Ubuntu's (24.04), built with TF-A.
- **The H618:** `orangepi_zero3` is in neither (Debian sid's 2025.01 list has no such board), so it is built from U-Boot's and TF-A's sources.

U-Boot and TF-A are free software (GPL-2.0+ and BSD-3-Clause). Neither enters the repository: a script fetches or builds them on the maintainer's machine, as `scripts/fetch_uboot.sh` does for QEMU's.

## Plan

1. **The script,** `scripts/board_uboot.sh <board>`:
   - for each board, one pinned build: the H5's from Debian's package, checked by SHA-256, or both built from pinned tags of U-Boot and TF-A in a pinned container image, whichever proves simpler to keep;
   - a cache like `fetch_uboot.sh`'s, without root; it prints the path of `u-boot-sunxi-with-spl.bin`.
2. **U-Boot's settings checked,** and changed only where needed:
   - the UEFI loader on;
   - the card first;
   - no network boot methods (DHCP, PXE, TFTP), as [351-KRN-0195](../issues-done/351-KRN-0195-no-firmware-network-boot-before-the-loader.done) asks of firmware.
3. **The image:** `make_usb_image.py --board <board>`, or a step after it:
   - the boot partition moved to start after U-Boot's last byte, at a 1 MiB boundary (4 MiB leaves room for a larger U-Boot), its FAT's hidden-sectors field set to match, the log and store partitions after it as now;
   - the SPL written at 8 KiB;
   - `check_image` and a new check refusing an overlap of U-Boot and any partition.
4. **Tests on the host:** the layout of each board's image (the MBR valid, the SPL's eGON header at 8 KiB, nothing overlapping). QEMU has neither SoC, so the boot itself is tested only on the boards.
5. **THIRD_PARTY.md:** the script, the sources and their licences, not the files.
6. **`build_and_write.sh`:** the board's word, so that one command writes the card, as for the MacBook's stick. Writing a card stays the maintainer's act.

## Acceptance criteria

- `scripts/board_uboot.sh` gives the same bytes on two runs, or names what differs.
- The host tests of both boards' layouts pass.
- On each board, U-Boot's banner on UART0 is followed by our loader's lines (100004-MOB-0002, 0003).

## Progress (2026-11-10)

**Plan steps 1 to 5 made** (`claude/MOB-boards`):
- **`scripts/board_uboot.sh <board>`:**
  - the pinned tags U-Boot v2026.10 (`55084065`) and TF-A v2.15.0 (`da738d5e`), each checked against its commit;
  - built into a cache with each tree's commit time and a fixed locale;
  - `--fetch-tools` unpacks Ubuntu's `bison`, `swig` and `python3-pyelftools` without root;
  - the network is off in U-Boot (`CONFIG_NO_NET`), so `boot_targets` is `fel mmc0 usb0` on the Zero 3 and `fel mmc_auto usb0` on the H5, and the script refuses a build that still has `pxe` or `dhcp`. The UEFI loader stays on.
- **The bytes** (Ubuntu 24.04, `gcc-aarch64-linux-gnu` 13.3.0): the same in a second build of the same tags in another directory (the base defconfigs, by a helper session) and again through the script with the network off:
  - Orange Pi Zero Plus 2: 761269 bytes, SHA-256 `f229497a…6b9e1f`;
  - Orange Pi Zero 3: 815961 bytes, SHA-256 `6902f66c…7d24`;
  - both end below 1 MiB at 8 KiB, so a boot partition at 4 MiB leaves room.
- **`scripts/board_image.py`:**
  - the image's three partitions moved past U-Boot, the boot one to 4 MiB;
  - the FAT volumes' hidden sectors set to their new start;
  - U-Boot at 8 KiB after its `eGON.BT0` header is checked;
  - `check_image` and an overlap check.
- **Tests:** `tests/board_image_test.py` in `host_tests.sh`.
- **THIRD_PARTY.md:** both sources, their licences, and what a card given to others must carry.

**Read in the sources, not seen on a board** (MC-12.3):
- the ROMs read 8 KiB first; their second place is 128 KiB on the H5 and 256 KiB on the H616/H618;
- U-Boot installs its own control tree as the EFI device tree;
- the H5 has a GOP only with an HDMI monitor whose EDID is read; the H618 has none (`CONFIG_VIDEO` off);
- the H618's SPL sets the AXP313a's DRAM rail (1.1 V) before the DRAM;
- TF-A reserves 0x40000000 to 0x4003FFFF there;
- **TF-A hands over to U-Boot at EL2, and U-Boot starts EFI applications at EL2:** see 100004-MOB-0002.

**The first cards** (2026-11-10, the tree of d012aa0f, `main` b3fc86bb with this track's text) are on the maintainer's machine:
- `dist/boards/mind-orangepi-zero-plus2-h5.img`, SHA-256 `467d236c…af88e94`;
- `dist/boards/mind-orangepi-zero3.img`, SHA-256 `5e99172997…2861f1e`.

They show U-Boot, the loader and the kernel's first lines, and stop at `INIT STARTED` until the kernel leaves EL2 (100004-MOB-0002).

**Left:** plan step 6 (`build_and_write.sh`'s board word with `link.conf`), and the boards' runs.

## Related

[100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [scripts/fetch_uboot.sh](../scripts/fetch_uboot.sh), [scripts/make_usb_image.py](../scripts/make_usb_image.py), [100000](100000-a-phone-the-pinephone-first.md) (stage 1: Tow-Boot on the phone's eMMC and our card).
