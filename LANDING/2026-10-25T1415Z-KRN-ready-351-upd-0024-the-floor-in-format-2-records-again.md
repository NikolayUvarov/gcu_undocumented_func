# Ready again: 351-UPD-0024, the floor in format 2 records, the image's floor fixed

**From:** KRN (for UPD) · **To:** LANDING (KRN) · **Written:** 2026-10-25 14:15 UTC · **Concerns:** 351-UPD-0024

**Ready:** `claude/youthful-mendel-mf1soy` 2cffab4f

- **What round 35 returned:** an image made with a channel took the channel's minimum as its floor whatever slot A's manifest named.
- **The fix,** 2cffab4f on top of b019ce4c: the image takes the floor only as the updater would raise it, when slot A's manifest names that version or above, and 0 otherwise; slots.md (EN, RU) say so.
- **Tested** (QEMU, local, x86): `make_usb_image.py --no-build --force` with `tests/usb_image_smoke.py` (the "x86: USB image" group), PASS. The rest as in the earlier request (`LANDING/2026-10-25T1327Z-…`).
- **In `fast-test`:** ebaeef4e.
