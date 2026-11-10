# 100004-MOB-0005 — The Allwinner MMC controller: the microSD on both boards, the H5's eMMC

**Type:** driver (the phone's own chip) · **Owner:** `MOB` · **Priority:** P1 · **Status:** open · **Blocked by:** [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md) or [0003](100004-MOB-0003-the-first-boot-on-the-h618.md) (a board that boots) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md); [100000](100000-a-phone-the-pinephone-first.md), stage 2 (the phone's microSD and eMMC) · **Constitution:** MC-1.5, MC-2.3, MC-4.1, MC-12.1, MC-12.3

## Problem

Once the loader has run, MIND Core on a board has no disk:
- the boot images are in memory, and the system runs;
- but the log is not kept, `link`'s `put` has nowhere lasting to write, and a release cannot reach a slot;
- the boards have no PCIe, no VirtIO and, until `usb_host` drives their EHCI, no USB storage.

**The controller** is the same IP block on the A64, the H5 and the H618 (Linux's `sunxi-mmc`), at different addresses:
- the H5 and the A64: mmc0 0x01C0F000, mmc1 0x01C10000, mmc2 0x01C11000;
- the H618: mmc0 0x04020000.

It moves data by its own DMA (IDMAC) through a descriptor list.

## Plan

1. **A service, `sunxi_mmc`,** serving `block.wit` as the other block drivers do. Its parts:
   - **binding:** by the node's `compatible` (`allwinner,sun50i-a64-mmc`, `…-emmc`, `allwinner,sun50i-h5-mmc`, `allwinner,sun50i-h616-mmc`), its registers and interrupt lent by `init` ([100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done));
   - **the card:** found and set up (CMD0, CMD8, ACMD41, CMD2, CMD3, CMD9, CMD7, a 4-bit bus, high speed where the card says so); the eMMC with its own sequence and an 8-bit bus;
   - **transfers:** by IDMAC from DMA memory the kernel gives, each with a deadline, a failure answered as `io` (000-KRN-0228's `Outcome`);
   - **the clock:** first as U-Boot left it, with the card's clock divided in the controller itself. A clock the controller cannot set itself waits for the clock controller's service, a later task.
2. **What `init` needs** (a `KRN` request when the driver runs): the service started for each enabled MMC node, the boot disk found by its identity as on x86, the store's partition.
3. **Tests:**
   - host tests of the command sequence and the descriptors against a model;
   - on each board, a file written to the card read back after a reboot;
   - on the H5, the eMMC read, and written only in a partition this test made.

## Acceptance criteria

- **On both boards:** the system log kept on the card across a reboot, and a release put over the link taken up from a slot.
- **On the H5:** the eMMC's size and identity read; a block written and read back in a test partition.
- **What is not tested** is listed: SDIO, UHS speeds, the A64 itself (MC-12.3).

## Related

[100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md), [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md) (deadlines in block drivers), [100000](100000-a-phone-the-pinephone-first.md).
