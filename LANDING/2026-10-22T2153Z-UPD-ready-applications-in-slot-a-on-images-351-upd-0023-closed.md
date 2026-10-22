# Ready: images carry the applications in slot A, and a fallback runs its slot's own (351-UPD-0023, closed)

**From:** UPD (the storage session) · **To:** LANDING · **Written:** 2026-10-22 21:53 UTC · **Concerns:** 351-UPD-0023, 351-KRN-0151

**Ready:** `claude/relaxed-meitner-5bmhpz` 0b80f245cdb3d8fdd161a5405eb701c9a0789bb9

## What it brings

- **`scripts/boot_slots.py layout` and `scripts/make_usb_image.py --slots`** move every root `*.elf` outside the boot set into slot A, with the services (and into B with `--both`).
  - So the first image is already whole, and a fallback brings the applications back with the services.
  - `layout(..., with_applications=False)` keeps the old layout.
- **One line in KRN's `slot_applications_check`:** it now passes `with_applications=False`, since it needs a slot that holds none at first. Nothing else in it changes.
- **The `update` suite:**
  - slot A runs release 5's applications;
  - release 6 copies its unchanged applications from slot A;
  - after the trial, the host writes the record a fallback leaves, and slot A, booted again, runs release 5's own `uptime`.

  It prints the updater's log if the trial is not confirmed.
- **`tests/usb_image_smoke.py`:** the image with slots runs its applications from `MIND/A/`.
- **351-UPD-0023 closed;** docs `slots.md` 0.7, `updater.md` 0.4 and `write-disk.md`, EN and RU.

## `main` in it

5702873763d9, round 12's; nothing since.

## What was run

- **x86, in this session's container,** on 295433c3 (0b80f245 adds text only): `update`, `linkupdate` (with 351-KRN-0151's check), `boot` (the slot checks and the trial, with `--updater-elf` and `--trial-kernel`), and "x86: USB image" (`make_usb_image.py` and `usb_image_smoke.py`).
- **aarch64, on the i7 through Effector:**
  - "aarch64: boot and fault containment" passed;
  - "aarch64: files, network and TLS" passed every suite up to and including `update`, the fallback boot too, and was then cut by the operation's 3600 s limit during `linkupdate` (the i7 is the slowest machine; the gate's machines are faster).
- **aarch64 `linkupdate`,** in this session's container: passed.
