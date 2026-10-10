# 351-UPD-0016 — Images built with slots

**Type:** update (images) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** — ([351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), the updater, done) · **Main task:** [351](351-self-update.md) · **Roadmap:** track C · **Constitution:** MC-9.1, MC-9.3

Split from 351-UPD-0007, which took it over from 351-UPD-0006.

## Problem

The USB images and the volumes of the QEMU suites boot from the volume's root. Such a system has no update zone, so the updater has no slot to fill. Only an image laid out with `scripts/boot_slots.py layout` can be updated, and its slot A shows no version (it keeps no channel), so the updater takes it as version 0.

## Plan

- `scripts/make_usb_image.py --slots`: the build in slot A, confirmed, with an empty slot B, as `boot_slots.py layout` does. Later this becomes the default.
- With `--release DIR --channel NAME`, the image is made from a staged and published release: its channel file is kept in `MIND/A/CHANNEL`, so the updater knows the running version.
- `update.txt` and the policy line for `updater`, from options of the image script.
- The programs and guides that name `kernel.elf` at the root follow: `files`, the disk-writing guides.

## Acceptance criteria

- An image made with `--slots` boots from slot A in QEMU, and `vfs_server` names slot B as the update zone.
- Made from a release, the updater reports its version at start.
- The USB image check passes with it.

## Related

[351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), [docs/update/slots.md](../docs/update/slots.md), [docs/update/updater.md](../docs/update/updater.md).
