# 351-UPD-0023 — A release carries its applications in its slot, so its services and applications move and fall back together

**Type:** update (service, images) · **Owner:** `UPD` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** in progress (the updater's part built and tested on x86 in this session's container; the loader's part waits for `KRN`) · **Blocked by:** `KRN` for the loader's and `scripts/link_update.py`'s parts ([requests-KRN.md](requests-KRN.md)) · **Main task:** [351](351-self-update.md) · **Roadmap:** track C · **Constitution:** MC-9.2, MC-9.3, MC-12.4

From the link track's report and request "An update through the link replaces the boot services but not the applications, and they drift apart" (`internal-dev-messages`, `ALL/2026-10-22T0428Z-LNK-…`), which asked `UPD` for a decision.

## Problem

- **What the updater stages.** A slot gets the kernel, the boot services (`BOOT_FILES` in `common/abi.rs`), the manifest and the channel. The applications stay at the volume's root, shared by both slots, and the updater never touches them ([slots.md](../docs/update/slots.md), "Not provided yet").
- **What the maintainer saw on the MacBook Pro (2026-10-22).** After the first update through the link, `wm`'s Settings could not set the time zone: "shell did not answer".
  - The disk's applications came from `fast-test` b559bdf, with `shell.wit` 1.9 (`set-zone`, 000-APP-0099).
  - The release (6758fcb) had `shell.wit` 1.8. Its older `shell.elf` went into the slot; the newer `wm.elf` stayed at the root and called a function the shell did not know.
- **Either direction.** A release can put a newer service under an older application, and a fallback can put an older service under an application left newer. Each image is bound to the interface schemas it was built with (MC-9.2), but the pair that runs is not the pair that was built and signed together. A fallback does not return to a whole configuration either (MC-9.3).

## Decision (2026-10-22)

Of the link track's two options, the first: **the release carries its applications, in its slot.** The second, the updater refusing a release whose interfaces are older than the root's applications, is not taken.
- It would stop the update rather than make it whole: the root's applications could then change only by rewriting the disk.
- With the applications in the slot, a fallback returns them with the services, and nothing is left to compare.

## Plan

- **The updater (`UPD`).** It stages every application the release's manifest lists into the slot, next to the boot files:
  - an application is a listed `*.elf` outside `BOOT_FILES` and other than `kernel.elf`;
  - each is checked by size and SHA-256 as a boot file is, logged as one, and resumed as one;
  - a file whose digest matches one already on the volume (the running slot's or the root's) is copied from it, not fetched;
  - the licences, the voice model and `data/` stay at the root, shared: they are data, not code bound to an interface;
  - `verify_other` checks the slot's applications too, so `apply` refuses a slot missing one.
- **The loader (`KRN`, requested).** On a disk booted from a slot that holds applications, it opens an application from the booted slot only, as it opens a helper since 109-KRN-0133. A slot that holds none (staged before this change) keeps the root's, as today.
- **The link (`KRN`, `scripts/link_update.py`, requested).** It puts the blobs of the applications the release changed with the boot files', since the updater reads them from the same `blobs/` directory. An unchanged one is copied from the volume and needs none.
- **Images.** `scripts/boot_slots.py layout` and `make_usb_image.py --slots` put the applications into slot A (and B with `--both`), and keep a copy at the root for a system booted from the root.
- **The bootloader.** Check that it boots a slot with applications in it: it verifies by name only what it loads.
- **Checks.**
  - The `update` suite: release 6's applications are staged in slot B; the trial boot runs them from slot B and the rollback runs release 5's from slot A, and the suite tells which copy ran.
  - Host tests for the split of a manifest into boot files and applications.
- **Docs.** [slots.md](../docs/update/slots.md) and [updater.md](../docs/update/updater.md) and their Russian copies: the layout table, the staging, the fallback, and "Applications in the slots" taken out of "Not provided yet".

## Meanwhile

The link track builds every release it puts through the link from a tree at least as new as the disk's applications (its message of 2026-10-22). That is a practice, not a guarantee: a fallback still runs the root's applications on the older slot.

## Progress (2026-10-22)

- **The updater's part is built.**
  - `slot_files` keeps the kernel, the boot services and the applications of a manifest; `plan::kind` splits them, with a host test (`tests/updater_host.rs`).
  - `fetch` stages the applications with the boot files. An application the volume holds as listed, in the running slot or at the root, is copied whole, including after a cut copy; otherwise it is fetched from `blobs/`.
  - `apply` and `rollback` refuse a slot that holds some applications but not every one listed, or one the manifest does not list. A slot with none passes, as before.
- **Tested on x86** in this session's container, on the branch with `main` 0a6be388:
  - the `update` suite: release 6 differs from 5 in `uptime.elf`. Slot B then holds all 54 applications as listed; only `uptime.elf`'s blob was asked of the server, and the others were copied from the root. The 95 files took about 35 s, the copies about 10 s.
  - the `linkupdate` suite: the link put only the boot files' blobs (43 files), and the update still went through, its applications copied from the root.
- **Not done yet:** aarch64; the images (`boot_slots.py layout`, `make_usb_image.py --slots`); the loader and `link_update.py` (`KRN`).

## Acceptance criteria

- [x] The updater stages, verifies and copies the applications as above; host tests pass.
- [ ] With the loader's part (`KRN`), the `update` suite shows the trial running release 6's application and the rollback release 5's, on x86 and aarch64.
- [ ] The images and `scripts/link_update.py` carry the applications in the slot.
- [ ] slots.md and updater.md (EN and RU) say where the applications are and what a fallback returns.

## Related

- [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done): slots A and B.
- [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md): helpers from the booted slot, the loader's rule this extends to applications.
- [750-UPD-0019](750-UPD-0019-signed-application-packages.md): applications installed apart from a release; this task covers only those a release carries.
