# 352 — Installable media: an installer ISO for PCs, and images and a package for ARM boards and phones

**Type:** main task · **Owner:** `UPD` (the kernel session), with `PRT`, `KRN`, `MOB` and `DRV` for their parts · **Priority:** P2 · **Status:** open (plan, 2026-11-20) · **Blocked by:** — · **Roadmap:** stage V "Update and distribution" · **Constitution:** MC-3.12, MC-9.1, MC-9.2, MC-9.4, MC-9.5, MC-9.7, MC-9.8, MC-12.1, MC-12.3

## Problem

The maintainer (2026-11-20): to distribute MIND Core, it needs:
- an ISO that installs it on a computer;
- an image or a package that installs it on ARM devices such as the Orange Pi boards and phones.

**What exists** (read on `main` `48e5870ae0`):
- **x86-64 and aarch64 UEFI:**
  - `04_make_usb_image.sh` builds `dist/mind-core-usb.img`, a raw MBR disk with the boot volume, `MIND LOG` and `MIND STORE`; with `--slots`, slots A and B ([docs/write-disk.md](../docs/write-disk.md));
  - a host writes it to a stick or a spare disk: `scripts/write_usb_linux.py`, `05_write_usb_windows.ps1`, and [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md) for an internal disk of the Linux host.
- **ARM boards:** [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md) builds a card for each of the two Orange Pi boards, with U-Boot and TF-A before MIND Core's image.
- **Phones:** [100000](100000-a-phone-the-pinephone-first.md). Tow-Boot is installed on the PinePhone by its own installer, and MIND Core's image goes on a card.
- **Releases:** `scripts/release.py` stages, signs and checks a channel, and `bundle` makes one ZIP of what a device of an architecture needs ([docs/update/publishing.md](../docs/update/publishing.md)). The updater stages such a release in the slot that did not boot ([351](351-self-update.md)).

**What is missing:**
- **For PCs:**
  - there is no ISO;
  - nothing installs MIND Core onto the machine it runs on. Every disk is written by another machine.
- **For ARM devices:**
  - each board's card is made from a local build, not from a release, and each board's details live in its script;
  - no phone or board installs MIND Core onto its own eMMC;
  - nothing lists in one place which device takes which firmware and which image.
- **For a release:**
  - no one command makes every artifact of one commit;
  - nothing gives one list of their sums, signed.

## Plan

The `UPD` track numbers its tasks. Each says what it needs from another track; those parts go to that track as requests.

| Task | What |
|---|---|
| [352-UPD-0047](352-UPD-0047-an-installer-iso.md) | **An installer ISO**, x86-64 and aarch64 UEFI: a hybrid ISO 9660 that boots from an optical disc, a USB stick or a virtual machine's drive. It runs a live system from a copy of its volume in memory. |
| [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md) | **The installer:** from the live system, MIND Core onto a disk of the same machine, with slots A and B, the log and the store, the release's signature checked. On ARM devices the same installer takes a card's system onto the eMMC. |
| [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md) | **Images for ARM boards and phones,** each built from the one aarch64 release and a board's entry: its firmware (U-Boot with TF-A, or Tow-Boot), its offsets and its device tree. 100004-MOB-0001's two boards come first, the PinePhone next. |
| [352-UPD-0050](352-UPD-0050-a-releases-artifacts-from-one-commit.md) | **A release's artifacts from one commit:** each of these, with one signed list of their SHA-256 sums: <ul><li>the ISOs;</li><li>the USB images;</li><li>the board images;</li><li>the update package, `release.py bundle`, the package a device that already runs MIND Core takes through its updater or the Effector agent.</li></ul> Reproducible where the build is. |

**Order:** 0047 and 0048 first, as the maintainer asked for the ISO first; 0049 with MOB's boards; 0050 when the artifacts exist.

**What stays a person's act:**
- writing a medium;
- the choice of the disk to erase, confirmed at the machine;
- Secure Boot's keys on a real machine;
- publishing a release ([issues-human](../issues-human/README.md)).

## Acceptance criteria

- **QEMU, x86-64 and aarch64:**
  - the ISO boots as an optical disc and as a disk;
  - its installer installs onto an empty virtual disk;
  - the installed disk boots on its own, its slot A confirmed, and takes an update through the updater.
- **A real PC** boots the ISO from a USB stick and installs onto its internal disk. That machine and build are named (MC-12.1).
- **An Orange Pi board and a phone** boot an image made by 352-UPD-0049. The phone installs onto its eMMC with 352-UPD-0048's installer.
- **One command** makes every artifact of a commit, with a signed `SHA256SUMS`. A second build of the same commit gives the same sums, except what the reproducibility check names.

Until each is shown, the profile says what is not done (MC-12.3).

## Related

- [351](351-self-update.md): slots and the updater; [350](350-signed-boot-images.md): signed boot images; [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md): the host's writer.
- [100004](100004-two-allwinner-boards-before-the-phone.md), [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md): the boards' card; [100000](100000-a-phone-the-pinephone-first.md): the phone; [205](205-aarch64-boards.md): aarch64 boards with UEFI.
- [docs/write-disk.md](../docs/write-disk.md), [docs/update/publishing.md](../docs/update/publishing.md).
