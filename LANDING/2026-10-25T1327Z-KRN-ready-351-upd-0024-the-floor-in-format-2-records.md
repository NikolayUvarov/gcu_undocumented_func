# Ready: 351-UPD-0024, the version floor in the boot records (format 2); format 1 not read

**From:** KRN (for UPD) · **To:** LANDING (KRN) · **Written:** 2026-10-25 13:27 UTC · **Concerns:** 351-UPD-0024

**Ready:** `claude/youthful-mendel-mf1soy` b019ce4c

- **Its base:** `main` 79bc36ed merged.
- **What it does** (the maintainer, 2026-10-25: no compatibility with our own older formats before the first release):
  - records of format 2 hold the floor (bytes 24–31); the newer valid record's floor counts, and the bootloader's count of a trial's tries carries it on;
  - a record of format 1 is not read: the bootloader, the updater and `boot_slots.py show` name it, and a disk with both such records boots as one without a valid record;
  - the updater raises the floor with a record of its own (`plan::raise`), and every record it writes carries the floor on;
  - `MIND/FLOOR` is gone, from the bootloader, the updater, the update zone and the tools. An image starts at its channel's minimum.
- **Tested on this tree** (QEMU, local):
  - x86: `boot` (`version_floor` with the format 1 disk), `update`, `linkupdate`;
  - aarch64: `aarch64_smoke` (`version_floor` too), `update`, `linkupdate`;
  - `scripts/host_tests.sh`: PASS.
- **For the other tracks:** a disk written before this has records of format 1. It boots as one without a valid record, and is written again with `make_usb_image.py --slots`. The MacBook Pro's stick is one such disk.
- **In `fast-test`:** 4d216d5b.
