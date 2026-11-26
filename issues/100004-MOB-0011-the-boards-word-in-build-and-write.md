# 100004-MOB-0011 — The board's word in `build_and_write.sh`: one command builds a board's card and opens the writer

**Type:** script · **Owner:** `MOB` · **Priority:** P1 · **Status:** in progress (2026-11-26: made and tested on the host; the maintainer's first write of a card left) · **Blocked by:** for the last criterion, a card and the maintainer's machine ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-9.7, MC-12.1, MC-12.3

**Split from** [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md), its plan step 6.

## Problem

`scripts/build_and_write.sh <disk number>` is the maintainer's one step from WSL to a written USB disk ([docs/write-disk.md](../docs/write-disk.md), section 5): it builds, packs, opens the elevated writer, then puts the files its options ask for on the disk. It builds only the x86 image. A board's card took four commands by hand: the aarch64 build, its image, `board_image.py` with the firmware from `board_uboot.sh`, then the writer with `-Image`.

## Plan

1. **`--board <name>`,** a name of the catalog (`scripts/boards.toml`, [352-MOB-0010](../issues-done/352-MOB-0010-a-catalog-of-devices-for-board-image.done)):
   - `ARCH=aarch64 ./02_build.sh`, then `make_usb_image.py --arch aarch64`;
   - `board_image.py --board <name>`, whose entry's script gives the firmware, into `dist/boards/mind-<name>.img`;
   - with `--image <file>`, the card is made from that aarch64 image, its `MANIFEST` beside it, as `--image` asks today;
   - the writer and every check it makes of the disk unchanged: writing stays the maintainer's act, after `ERASE <n>`.
2. **After the write,** only what fits a board:
   - `link.conf` with `uart`, at 115200, for `--link` and the options that imply it: `uart_serial` keeps the rate the firmware left (100000-KRN-0221, [docs/link/mindlink.md](../docs/link/mindlink.md));
   - `--mode=` as for a PC;
   - no proprietary files, desktop pictures or `mac/startup-disk.sh`.
3. **Refused with `--board`:**
   - a name the catalog lacks;
   - the options of a PC's disk and of the MacBook's Wi-Fi (`--keep-*`, `--machine`, `--old-disk`, `--new-device-key`, `--wifitest`, `--wifi-ofdm`, `--wifijoin`);
   - a link rate other than 115200.
4. **Tests** in `tests/build_and_write_args_test.py`, with a stand-in repository whose build, packer, card maker and PowerShell only record what they are asked.
5. **docs/write-disk.md** and its Russian version.

## Acceptance criteria

- [ ] `--board` refuses a name the catalog lacks, the PC's options and another link rate, before anything is built.
- [ ] In the stand-in repository: the aarch64 build, its image and the card in that order, the writer given the card, `link.conf` with `uart` at 115200; nothing of a PC's steps.
- [ ] A PC's disk is made as before (the same test).
- [ ] The host tests pass in `scripts/host_tests.sh`.
- [ ] **On the maintainer's machine** (the maintainer's act): one command builds a card, the writer writes it to a card in a USB reader and reads it back. Not done: no card has been written with it.

## Related

[100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md), [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md) (`uart` in the card's `link.conf`), [docs/write-disk.md](../docs/write-disk.md).
