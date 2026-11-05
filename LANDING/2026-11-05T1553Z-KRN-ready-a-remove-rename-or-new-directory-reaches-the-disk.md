# Ready: a remove, a rename or a new directory reaches the disk at once (000-KRN-0212)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 15:53 UTC · **Concerns:** 000-KRN-0212 (closes once landed), LNK's and EFF's requests

**Ready:** `claude/000-KRN-0212-vfs-flush` a77e66b86fd141328ca089f7e8b487531f61e00a

It contains `main` as of now (cde4a52c).

## What it brings

- **`vfs_server`:**
  - `Remove` and `Rename` flush their volume when they are done;
  - `OpenDir` flushes when it made a directory (`Volume::changed`).
  
  A failed flush is the request's error.
- **The check,** in `linkupdate_suite`: `link` flushes nothing of its own, through a put's rename or its `mkdir`, `mv` and `rm`. The host reads each from the image as soon as `link` answers.
- **Docs:**
  - `docs/profile/threat-model.md`, the power-loss row;
  - `docs/profile/evidence.md`;
  - `docs/write-disk.md` 1.7 and `docs/write-disk_RU.md` 1.7.

## Tested

- **Without the fix** (`claude/000-KRN-0212-check-alone` ea4790c4, acc-1, x86): the check failed. The put's last file was not on the disk under its name.
- **With it** (6b86e4cf):
  - acc-1, x86: `the updater and an update through the link` passed;
  - acc-2, aarch64: `an update through the link` passed.
- **On ad906c8b:** `x86: files and block writes` (acc-1) and `aarch64: files and the block store` (acc-2) passed.
- **Host, in this session's container:** `tests/fat_host.rs` (18) and `tests/vfs_zone_host.rs` (7).
