# 352-MOB-0010 — A catalog of devices for `board_image.py`, and a card from a boot volume given to it

**Type:** image, script · **Owner:** `MOB` · **Priority:** P2 · **Status:** in progress (2026-11-26) · **Blocked by:** — · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md), for [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md) · **Constitution:** MC-9.1, MC-9.7, MC-12.1, MC-12.3

**Asked by:** the kernel session for `UPD`, 2026-11-20 (`requests-MOB.md`, request 1; message `MOB/2026-11-20T1931Z-KRN-a-catalog-of-devices-for-352-upd-0049.md`).

## Problem

[352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md) makes each ARM device's image from one release. It needs one place that says what each device takes. Before this task:
- **the two Orange Pi boards' details were in two scripts** ([100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md)):
  - `scripts/board_uboot.sh`: U-Boot's and TF-A's tags, commits and URLs, each board's defconfig and TF-A platform, the bytes known;
  - `scripts/board_image.py`: the firmware's offset on the card, its header, the boot partition's start;
- **a card was made only from a local build's image,** not from a release's boot volume;
- **nothing said where the firmware goes for a boot from the eMMC,** which 352-UPD-0048's installer needs.

## Plan

1. **The catalog, `scripts/boards.toml`,** one entry per device:
   - the device, its SoC, its architecture (the image and bundle it takes);
   - the firmware: the script that builds it, its sources (tag, commit, URLs, licence), its file, its header, its SHA-256 and the toolchain that gave those bytes;
   - the card: where the firmware goes (on the card at 8 KiB, or on the device's own flash), the boot partition's start, the device tree U-Boot hands over, and the tree the host tests read in its place;
   - the eMMC: where the firmware goes for a boot from it, or that the device has none.
2. **The facts moved, not copied:**
   - `board_uboot.sh` takes the board's build and the sources from its entry (`board_image.py --board <name> --shell`);
   - `board_image.py` takes the offset, the header and the boot partition's start from it.
   - `THIRD_PARTY.md` keeps its record of the sources, and a host test checks it against the catalog.
3. **`board_image.py --board <name>`:**
   - `--image`: a local build's image, or a release's `mind-core-<version>-aarch64.img`, whose boot partition is at 1 MiB;
   - `--volume`: a boot volume alone, a FAT image at 1 MiB as `release.py` makes one, with the release's date (`--date` or `SOURCE_DATE_EPOCH`) for the log volume, so that one volume gives the same card;
   - the firmware from the entry's script unless `--firmware` names a file; its SHA-256 is said beside the catalog's;
   - `--list` names the entries.
4. **The entries:** the Orange Pi Zero Plus 2 H5 and the Orange Pi Zero 3.
5. **Host tests** in `tests/board_image_test.py`, which `scripts/host_tests.sh` runs.

**Not in this task:**
- **The check of a boot volume against the release's signed manifest,** and the record beside the image: `UPD`'s, in 352-UPD-0049.
- **The PinePhone's entry and card.** They wait for the phone (the maintainer, 2026-11-26: «MOB будет делать работу на стенде - это не телефон, а плата разработки с добавленными компонентами»). Request 2's item 1 stays in [requests-MOB.md](requests-MOB.md). An entry whose firmware is on the device's own flash is refused with a message saying so.

## Acceptance criteria

- [ ] `board_image.py --board <name>` makes each entry's card from a local build's image, a release's image and a boot volume alone; a volume told it starts elsewhere than at 1 MiB is refused.
- [ ] Two cards from the same volume, date and firmware have the same SHA-256.
- [ ] `board_uboot.sh` and `board_image.py` hold none of the facts the catalog says; THIRD_PARTY.md's record agrees with it.
- [ ] The host tests pass in `scripts/host_tests.sh`.
- [ ] Request 1 is removed from `requests-MOB.md`.

## Related

[352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md), [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md), [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md), [100004](100004-two-allwinner-boards-before-the-phone.md), [100000](100000-a-phone-the-pinephone-first.md).
