# 352-UPD-0049 — Images for ARM boards and phones from the one aarch64 release

**Type:** image, script · **Owner:** `UPD` (the kernel session), with `MOB` (the boards' and the phone's images, requested) · **Priority:** P2 · **Status:** open (plan, 2026-11-20) · **Blocked by:** [requests-MOB.md](requests-MOB.md) (a catalog of boards for `board_image.py`) · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-9.1, MC-9.2, MC-9.7, MC-9.8, MC-12.1, MC-12.3

## Problem

An ARM board or phone without UEFI in its own flash boots MIND Core only from a card that also carries its firmware.

**What exists:**
- **The boards:** [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md) makes such a card for each of two Orange Pi boards:
  - `scripts/board_uboot.sh <board>` fetches or builds the board's U-Boot with TF-A;
  - `scripts/board_image.py` moves `make_usb_image.py`'s partitions past it and writes it at 8 KiB.
- **The phone:** the PinePhone ([100000](100000-a-phone-the-pinephone-first.md)) takes Tow-Boot, installed once by its own installer. MIND Core's image then goes on a card as it is.

**What is missing for distribution:**
- **One release, many devices.** Each board's image is built by hand from a local build, not from a release's signed boot volume. Nothing ties a board image to the release it carries.
- **Which device takes which firmware.** The two boards' details live in the two scripts, and the phone's in an issue. Nothing lists them in one place for the release to read.
- **A device that already runs MIND Core** needs no new card: it needs the update package. Which package fits which board is not written down.
- **Installing onto the eMMC** of a board or phone is not possible yet ([352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md)).

## Plan

1. **A catalog of devices (MOB, requested).** One entry per board or phone, read by `board_image.py --board <name>`:
   - the SoC;
   - the firmware: the script that fetches or builds it, its source and version, and its SHA-256 (fetched on the maintainer's machine, never committed);
   - where the firmware goes: on the card (U-Boot with TF-A, at 8 KiB) or on the device's own flash (Tow-Boot on the PinePhone's SPI);
   - the boot partition's start, and the device tree U-Boot hands over.
   - The first entries: the Orange Pi Zero Plus 2 (H5) and Zero 3 (H618), then the PinePhone.
2. **The images come from a release (UPD).** `board_image.py` takes the release's aarch64 boot volume, the same FAT image as the aarch64 ISO's and USB image's (the same SHA-256):
   - its files are checked against the release's signed manifest before the image is written (MC-9.1);
   - the image's name carries the release and the device: `mind-core-<version>-<board>.img`;
   - a record beside it names the release, the firmware's source, version and SHA-256, and the commit of the scripts (MC-9.7).
3. **The firmware is not signed by the release.** U-Boot, TF-A and Tow-Boot are third-party, and the release's keys do not vouch for them (MC-9.2). The image's record says so, and `THIRD_PARTY.md` names their sources and licences.
4. **Proprietary files stay off the image** (AGENTS.md section 3): a board's Wi-Fi firmware, for example, is copied by the script onto the written card, under `data/firmware/`, on the maintainer's machine only.
5. **The update package for a device that runs MIND Core already** is the aarch64 bundle of `release.py bundle`. The updater stages it in slot B, and the device's firmware stays as it is. The catalog says which bundle each device takes: one aarch64 bundle for all today.
6. **Onto the eMMC.** A card booted on a board or phone with eMMC offers 352-UPD-0048's installer. The catalog says where that device's firmware goes for an eMMC boot.

## Acceptance criteria

- [ ] `board_image.py --board <name>` makes each catalog device's image from one release's aarch64 boot volume, checked against its manifest. A boot volume that fails the check is refused.
- [ ] Two builds of a board image from the same release and firmware give the same SHA-256 (`tests/board_image_test.py`).
- [ ] An Orange Pi board boots the image made from a release, and takes the next release's aarch64 bundle through its updater. Shown on that board (MC-12.1).
- [ ] The PinePhone, with Tow-Boot, boots its image and installs onto its eMMC (with 352-UPD-0048). Shown on the phone (MC-12.1).
- [ ] docs/write-disk.md (EN and RU) lists each device, its image, its firmware and how it was shown. A device not yet shown is listed as such (MC-12.3).

## Related

[352](352-installable-media-for-pcs-and-arm-devices.md), [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md), [100004](100004-two-allwinner-boards-before-the-phone.md), [100000](100000-a-phone-the-pinephone-first.md), [205](205-aarch64-boards.md) (aarch64 boards with UEFI in their flash: they boot the aarch64 ISO or USB image instead), [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md).
