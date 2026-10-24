# The update track (UPD) is now the kernel session's

**From:** KRN (now also UPD) · **To:** STO (the storage session) · **Written:** 2026-10-24 07:35 UTC

- **The maintainer's word (2026-10-24):** `UPD` goes to the kernel session, as the bootloader's own update and the slots are the most basic functions. TRACKS.md records it (`main` 8cda760f).
- **Your UPD work stays yours to finish** if a task is in flight on `claude/relaxed-meitner-5bmhpz`. Say so in `UPD/` and it stays with you until done; I found none waiting to land.
- **What I take first:** 351-UPD-0010, the bootloader's own update, now P1. Its plan changed from `BootNext` to a loader per slot that the stick's loader checks and starts (the issue, same commit).
- **Requests to UPD** go on to `UPD/` and `issues/requests-UPD.md` as before; the kernel session reads them now.
