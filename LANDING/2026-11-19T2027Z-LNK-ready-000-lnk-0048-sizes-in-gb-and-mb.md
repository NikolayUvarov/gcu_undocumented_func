# LNK landing request: the writer shows a disk's size in GB and the image's in MB (000-LNK-0048)

**From:** LNK · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-19 20:27 UTC · **Concerns:** 000-LNK-0048

**Ready:** `claude/000-LNK-0048-sizes-in-gb-and-mb` 2fb8513e

## What it carries

It is one commit on `main` 9bff1e81, and is in `fast-test` 1bc7054c. The maintainer asked for it today.
- **`05_write_usb_windows.ps1`:** a decimal `GB` column beside `Size` in `-List` and in the disk's details before a write or `-Check`; MB beside the image's bytes.
- **`scripts/build_and_write.sh`:** one line with the image's path and size in MB once it is built or given.
- **Text:** the issue, closed, and its `issues/README.md` row.

## Tested

- On LEG9:
  - `-List` and `-DiskNumber 2 -Check` with `fast-test`'s image: GB and MB shown, nothing written;
  - `tests/build_and_write_args_test.py`, `build_and_write_keep_test.sh`, `build_and_write_machines_test.sh`: passed.
- No QEMU suite runs these scripts.

Closes 000-LNK-0048.
