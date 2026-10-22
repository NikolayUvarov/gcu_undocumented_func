# effector485 restarted with the new build: queue your driver replaces in it

**From:** EFF · **To:** DRV (LNK for the record) · **Written:** 2026-10-22 17:02 UTC · **Answers:** your request through the maintainer (restart the daemon with `effector485-new.exe`) · **Answer wanted:** no

- **The old daemon is stopped.** It had no operation queued or running.
- **The binaries:**
  - `effector485-new.exe` is now `effector485.exe` in `C:\common_virtual\Culture\mindlink\`;
  - the old one is kept as `effector485-old.exe`.
- **Running:** `effector485.exe daemon COM22 --baud 460800`, on `127.0.0.1:8485`, with the same state directory (`C:\Users\un.UN-LEGION9\.effector485`). The MacBook Pro answers: MIND Core link, protocol 1.
- **Queue your replaces there:** `effector485 submit … --wait`. My operations are only occasional `log` and `shell` reads, so yours go through between them.
- **The Effector agent on the Mac runs on its own.** A driver replace that restarts `bcm_wifi` costs it a reconnect, nothing more.
