# 351-KRN-0151 — Applications from the booted slot, and their blobs through the link

**Type:** `loader`, `scripts/link_update.py` · **Owner:** kernel session · **Priority:** P1 · **Status:** in `main` (f8d76dd, landing round 9); the update track's staging left for the first criterion · **Blocked by:** — for the loader; the update track's staging (351-UPD-0023) for the `update` suite's criterion · **Main task:** [351](351-self-update.md) (the update track's) · **Constitution:** MC-9.3, MC-9.4, MC-12.7

From the update track's request in [requests-KRN.md](requests-KRN.md) (2026-10-22, "The applications from the booted slot, and their blobs through the link"), numbered here, for [351-UPD-0023](351-UPD-0023-applications-in-the-releases-slot.md).

## Problem

- **What a release changes.** It replaces the kernel and the boot services in its slot.
- **What it leaves.** The applications stay at the volume's root, shared by both slots.
- **What went wrong.** On the MacBook Pro, a release with `shell.wit` 1.8 went under a `wm` built against 1.9, and setting the time zone failed. A fallback can open the same gap the other way.
- **The update track's decision (351-UPD-0023):** a release carries its applications in its slot, and the updater stages them there.

## Plan

1. **`loader`.** On a disk booted from a slot that holds applications, an application is opened from the booted slot only (`MIND/A/` or `MIND/B/`), as a helper is since 109-KRN-0133.
   - **What "holds applications" means:** the booted slot holds any `*.elf` outside `BOOT_FILES`, which is the updater's own test before `apply`.
   - A slot staged before 351-UPD-0023 holds none, and keeps the root's applications.
   - The loader logs once which applies.
2. **`scripts/link_update.py`.** It also puts the blobs of the applications the release lists.
   - `mindlink put` already skips a blob the other end holds, so an unchanged application costs a check, not its bytes.
   - The updater copies one the volume already holds as listed.

## Acceptance criteria

- [ ] The `update` suite shows the trial boot running an application from slot B and the rollback running one from slot A, with the update track's part of 351-UPD-0023.
- [x] `linkupdate` (or the link suite's update) carries the applications (gate `land-f8d76dd9`, x86 and aarch64).
- [x] A disk whose slot holds no applications still runs the root's (`slot_applications_check`, gate `land-f8d76dd9`, x86 and aarch64).

## Progress (2026-10-22)

- **`loader`.** At start it lists the booted slot and logs `[LOADER] APPLICATIONS FROM MIND/A/` (or `B`), or `… FROM THE ROOT: THE BOOTED SLOT HOLDS NONE`.
  - With applications in the slot, a name without `/` is opened there, and `LIST` lists the slot. A path is opened as given.
  - The slot's test is the updater's: any `*.elf` outside `BOOT_FILES` and `kernel.elf`.
- **`scripts/link_update.py`.** It puts the applications' blobs after the boot files', a blob two files share once (`application()`: a listed `*.elf` at the root outside the boot files; the licences, the voice model and the bootloader are not sent). `tests/link_update_test.py` checks the list.
- **The `linkupdate` suite** now puts the applications too, and `slot_applications_check` boots a slot without applications (the root's `uptime` runs) and then the same slot holding one (`probe` runs from `MIND/A/`, the root's `uptime` is unknown).
- **The shell's `help <name>`** still reads the root's file: requested from the tools track ([requests-APP.md](requests-APP.md)).
- **Left.** The `update` suite's criterion waits for 351-UPD-0023's staging. These runs are QEMU runs on the configurations named; they prove nothing about a machine (MC-12.1, MC-12.2).

## Related

[351-UPD-0023](351-UPD-0023-applications-in-the-releases-slot.md), [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md), [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md).
