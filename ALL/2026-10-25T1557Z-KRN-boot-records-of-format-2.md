# Boot records are of format 2 on `main`: a disk with records of format 1 is written again

**From:** KRN · **To:** ALL (UPD, LNK, EFF, DRV, the maintainer in particular) · **Written:** 2026-10-25 15:57 UTC · **Answers:** — · **Concerns:** 351-UPD-0024, `main` 3937e0a4 (round 36), closed in 448dea99 · **Answer wanted:** no

- **What changed:** `MIND/BOOT0` and `MIND/BOOT1` carry the version floor in bytes 24–31 (format 2). `MIND/FLOOR` is gone.
- **Format 1 is not read** (the maintainer's rule of 2026-10-25, AGENTS.md section 3: no compatibility with our own older formats before the first release).
  - The bootloader names such a record: `BOOT: RECORD MIND\BOOT0 OF FORMAT 1, NOT READ: WRITE THE DISK AGAIN`.
  - `boot_slots.py show` and the updater name it too.
  - A disk whose two records are both of format 1 boots as a disk without a valid record.
- **What to do:**
  - write the stick again from `main` (`make_usb_image.py --slots`), the MacBook Pro's included;
  - a test or a script that writes records by hand uses `boot_slots.record(..., floor=N)` (`scripts/boot_slots.py`).
- **Images** start at the minimum of the channel they come with when slot A's manifest meets it, at 0 otherwise.
- **Evidence:** the gate on 9f39f44c, every group of `scripts/ci_local.sh --list`, in QEMU on x86 and aarch64 (`version_floor`, `update`, `linkupdate`). Not run on hardware yet (MC-12.1).
