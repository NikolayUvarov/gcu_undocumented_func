# KRN: ready — the installer ISO's first step: make_iso.py and its checks (352-UPD-0047)

**Ready:** `claude/352-UPD-0047-installer-iso` 8237936bb2

**What it lands:** 352-UPD-0047 step 1 (the issue stays open: step 2, the volume in memory, and the rest).
- **`scripts/make_iso.py`:**
  - an ISO 9660 volume with one El Torito EFI entry, the boot volume: a FAT16 image of `make_usb_image.py`'s payloads under the build's signed manifest;
  - an MBR whose one EFI system partition is the same volume at 1 MiB;
  - every date is the build's, so the same build gives the same bytes; `check_iso` reads it all back.
- **`tests/make_iso_test.py`,** one line in `scripts/host_tests.sh`.
- **`tests/qemu_smoke.py`:** the `iso` suite (x86), added to the group `x86: keys, shell, tools`, about 2.5 min more:
  - written as a USB disk, the ISO boots to the shell from its volume;
  - as an IDE CD-ROM, the loader reads and checks the kernel and the services.
- **Docs:** docs/write-disk.md 1.9 (EN and RU), section 12, and the issue's progress.
- **No IDL or ABI change.** Nothing of it goes into an image.

**Tested** on `8237936bb2` (`main` 03c88c5144), in this session's container (QEMU 8.2.2, OVMF, x86, 4 CPUs):
- `tests/make_iso_test.py` 4 of 4;
- the `iso` suite PASS;
- two builds 2 s apart: the same SHA-256.

The rest of the group `x86: keys, shell, tools` is unchanged and runs in the gate.
