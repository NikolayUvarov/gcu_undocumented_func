# 351-APP-0088 — `files` reads the kernel from the slot that booted

**Type:** tools (`files`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track E · **Constitution:** MC-12.1

**Recorded by:** the update track (`UPD`), 2026-10-11, in `requests-APP.md`, for [351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done). Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

`files` reads `kernel.elf` and the bootloader to show that whole files come back through `vfs_server`. On an image made with `make_usb_image.py --slots` the kernel is in `MIND/A/kernel.elf` (or `MIND/B/` after an update), not at the root, so `files` reports `OPEN kernel.elf FAILED`.

## Plan

- Read `kernel.elf` from the root when it is there, else from the slot that booted (`BootInfo.boot_slot`), `MIND/A` or `MIND/B`.
- Say which one was read.

## Acceptance criteria

- On the slot image of `tests/usb_image_smoke.py`, `files` reads the slot's kernel whole and names its path.
- On the plain image, it reads the root's as before.

## Progress (2026-10-20)

- **`files/src/main.rs`:** reads `kernel.elf` from the root when it is there; otherwise, on a boot from slot A or B (`BootInfo.boot_slot`), `MIND/A/kernel.elf` or `MIND/B/kernel.elf`, and its line ends `(THE SLOT THAT BOOTED)`.
- **`tests/usb_image_smoke.py`, the slot image:** `files` reads `MIND/A/kernel.elf` whole (its size from the packed payload, the ELF magic). The plain image's `files_check` stays as it was.
- **Left:** the gate on this batch.

## Related

[351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done) (images with slots).
