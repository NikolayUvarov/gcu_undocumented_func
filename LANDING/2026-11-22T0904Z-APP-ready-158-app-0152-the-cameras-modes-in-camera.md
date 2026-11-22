# APP: ready to land, the camera's modes in `camera` (158-APP-0152)

**From:** APP · **To:** LANDING · **Written:** 2026-11-22 09:04 UTC · **Answer wanted:** in LANDING/

**Ready:** `claude/wizardly-franklin-kec1a9` a89db79d2fdfacd9c6b75e930addd34e8b9397c6

`main` ce4233d90d is in it. The commits since are APP's: one text-only one already on `main` (211f5a6fb2), and 158-APP-0152's.

**What it closes or advances:** 158-APP-0152 advances, on DRV's `video.wit` 1.1 (in `main` since round 169).
- `camera -m n` opens a mode at its own size and rate.
- `m` and Shift+`m` step through the offered modes, each with a frame buffer of its size.
- The mode in use, from `opened()`, is in `[CAMERA] OPENED …` and the window's title.
- A 1.0 gateway, and a camera with one offered mode, are named.
- The Ryzen's cameras are left, with DRV.

**Tested,** in this container, x86_64, 4 CPUs, on this tree:
- the `vfs` suite (8 PASS), whose camera check now steps the test pattern's three modes, takes a still at `-m 0` (640×480, on `ram:`), and refuses `-m 7`;
- DRV's `cameras` suite;
- `tests/camera_host.rs` (2);
- the aarch64 build. No aarch64 QEMU run.
