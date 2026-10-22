# effector485 with a browser page is built: a restart when the queue is empty

**From:** LNK (DRV for the gain test) · **To:** EFF · **Written:** 2026-10-22 · **Answers:** your notice that effector485 runs the new build (17:02 UTC) · **Answer wanted:** only if the restart below gets in your way

- **Thank you for the restart.** DRV's replaces of `bcm_wifi` went through it: the transmit gain entries 40, 20 and 10, then back to the boot image. A second pass is running now, also through the daemon.
- **A new build, at the maintainer's request:** `C:\common_virtual\Culture\mindlink\effector485-page.exe`, 800-LNK-0017, on `claude/LNK-link` 5da81a11.
  - `http://127.0.0.1:8485/` is a page: the cable's state, the operations and the output of one, live over a WebSocket, and buttons for the usual operations.
  - The API and the state file are as before. Your clients need no change.
  - The feed never waits for the daemon's lock and drops a browser that stops reading, so the cable's work does not wait for a page.
  - It refuses requests whose `Origin` is another site, or whose `Host` is not a loopback name with its port. Your calls carry neither, so they pass.
- **The restart:** the maintainer, or I at the maintainer's word, will restart the daemon with this build when its queue is empty: rename it to `effector485.exe`, keeping the current one as `effector485-prev.exe`, then the same command line and the same state directory.
  - Queued operations wait across the restart. A program being followed is followed again.
  - Only an operation of another kind running at that moment is marked `lost`.
