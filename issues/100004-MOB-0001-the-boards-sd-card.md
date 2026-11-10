# 100004-MOB-0001 — The boards' card: U-Boot with TF-A before MIND Core's image

**Type:** image, script · **Owner:** `MOB` · **Priority:** P1 · **Status:** in progress (2026-11-10) · **Blocked by:** — · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-1.3, MC-12.1, MC-12.3

## Problem

**What an Allwinner board boots.** Its boot ROM reads a first-stage loader (U-Boot's SPL) from fixed sectors of the microSD card. The SPL sets up the DRAM, then starts TF-A's BL31 and U-Boot. U-Boot's UEFI then starts `EFI/BOOT/BOOTAA64.EFI` from the card's ESP.

**What MIND Core's image has.** `make_usb_image.py` writes a GPT disk: the ESP with the loader and the slots, `MIND LOG`, the store's partition. Nothing in it is for a board's boot ROM.

**Where the two meet.** A GPT's partition entries take sectors 2 to 33 by default. The ROM's usual place for the SPL is 8 KiB in, sector 16, inside them. U-Boot's documentation and the ROM's second place (128 KiB) are to be read and checked first.

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
   - the SPL written where the board's ROM reads it, at an offset the GPT's entries do not cover;
   - the first partition placed after U-Boot's last byte;
   - a check that refuses an overlap.
4. **Tests on the host:** the layout of each board's image (the GPT valid, the SPL's eGON header at its offset, nothing overlapping). QEMU has neither SoC, so the boot itself is tested only on the boards.
5. **THIRD_PARTY.md:** the script, the sources and their licences, not the files.
6. **`build_and_write.sh`:** the board's word, so that one command writes the card, as for the MacBook's stick. Writing a card stays the maintainer's act.

## Acceptance criteria

- `scripts/board_uboot.sh` gives the same bytes on two runs, or names what differs.
- The host tests of both boards' layouts pass.
- On each board, U-Boot's banner on UART0 is followed by our loader's lines (100004-MOB-0002, 0003).

## Related

[100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [scripts/fetch_uboot.sh](../scripts/fetch_uboot.sh), [scripts/make_usb_image.py](../scripts/make_usb_image.py), [100000](100000-a-phone-the-pinephone-first.md) (stage 1: Tow-Boot on the phone's eMMC and our card).
