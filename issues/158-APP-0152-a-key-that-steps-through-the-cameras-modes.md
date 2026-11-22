# 158-APP-0152 — A key in `camera` and `wm`'s camera that steps through the camera's modes

**Type:** tools (`camera`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done in QEMU on x86, 2026-11-22; the Ryzen's cameras left) · **Blocked by:** — (`video.wit` 1.1's `modes` and `opened`, [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md), in `main` since round 169) · **Main task:** [158](158-video-capture.md) · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.4

**Asked by:** DRV, `APP/2026-11-21T1259Z-DRV-request-a-key-in-camera-that-steps-through-the-cameras-modes.md` on `internal-dev-messages`, at the maintainer's word of 2026-11-21: «переключение камеры должно быть доступно и из самого приложения, как устройства, так и её режимов». Numbered on 2026-11-21.

## Problem

- **The camera itself** can be switched from inside the program since [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md): `-c <n>`, `n` or Tab.
- **Its modes** cannot. `camera` opens the size and rate it asks for, and the gateway scales.

## Plan

Once `video.wit` 1.1's `modes(camera)` is on a branch to build against:

- **The mode in use is shown:** its size, its rate and the encoding the camera sends, in the log line `[CAMERA] OPENED …` and in the window's title.
- **`m` steps to the next mode the gateway offers** for the camera shown, and Shift+`m` to the one before.
  - Each step is `close` and `open` at the mode's exact size and rate, so it is given unscaled.
  - A mode that does not open leaves the one before, as `n` does for a camera.
  - A still or a recording keeps its mode, as it keeps its camera.
- **`-m <n>`** picks mode `n` of the list at the start; one the list does not hold is refused with the count.
- **A camera with one mode, and a gateway still at 1.0,** behave as now: `m` says so (`[CAMERA] ONE MODE ONLY`, `[CAMERA] THE GATEWAY LISTS NO MODES (VIDEO.WIT 1.0)`).
- **`wm`'s camera** is the `camera` program in a window: the same keys.

## Acceptance criteria

- **Host tests** (`tests/camera_host.rs`): stepping through a list of modes both ways, `-m` checked against the count, the title's words.
- **The `vfs` suite's camera check** against the gateway's test pattern with more than one mode, if 158-DRV-0083 gives it some. Otherwise one mode, named.
- **The Ryzen's USB cameras,** with DRV.

## Related

[158](158-video-capture.md), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md), [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md).

## Progress (2026-11-21)

- **DRV's `video.wit` 1.1** is on a branch, from DRV's message `APP/2026-11-21T1738Z-DRV-video-wit-1-1-is-on-a-branch-modes-and-opened.md`.
  - `modes(camera)` gives each mode's encoding, size, rate and `offered`, in a stable order.
  - `opened()` gives the index of the mode in use (65535 for a size none of them has), its facts and `scaled`.
  - The test patterns have three modes, 640×480 at 30, 320×240 at 10, 160×120 at 5. MJPEG modes are listed and not offered until 158-DRV-0084.
- **Written, with no system calls:** `camera/src/modes.rs`:
  - `-m <n>` checked against the list and what is offered;
  - `m` and Shift+`m` round the offered modes, skipping the others, from either end when the mode in use is none of the list's;
  - a mode in words, `640x480 at 30/s YUY2`.
- **Tested:** `tests/camera_host.rs`, `the_cameras_modes_stepped_and_named`.
- **Left:** the calls and the keys in `camera` once `video.wit` 1.1 is in `main` (no other track's unlanded branch is merged here), the `vfs` suite's check against the three test modes, and the Ryzen's cameras with DRV.

## Progress (2026-11-22)

- **`video.wit` 1.1 landed** in round 169, with DRV's 158-DRV-0082 to 0084.
- **Wired into `camera`:**
  - `-m n` picks a mode at its own size and rate, checked with `modes.rs`'s `chosen`;
  - `m` and Shift+`m` step through the offered modes with `step`. Each opens at the mode's own size and rate, with a frame buffer of that size, the place on the screen worked out again; the mode before is opened again if one does not open;
  - `[CAMERA] OPENED … : MODE 1 (OF 0 TO 2), 320x240 at 10/s SYNTHETIC`, from `opened`, and the window's title with the mode;
  - a 1.0 gateway (`modes` rejected) and a camera with one offered mode are named and left as they are;
  - `n` reads the next camera's modes.
- **A buffer given up** for another mode's drops its shared capability.
- **The `vfs` suite's camera check** steps the test pattern's three modes:
  - `m` from mode 1 (320×240 at 10) to 2 (160×120 at 5), `m` round to 0 (640×480 at 30), Shift+`m` back to 2, each `OPENED` at its own size;
  - a still at `-m 0` is 640×480, on `ram:`;
  - `-m 7` is refused with the count.
- **Tested** in this container, x86_64:
  - the `vfs` suite passes (8 checks). Its first run failed later on, counting `data/`'s files, as the still was there; it is on `ram:` now;
  - DRV's `cameras` suite passes with this `camera`;
  - `tests/camera_host.rs` (2).
- **Left:** the Ryzen's two cameras, with DRV.
- **Docs:** `docs/tools/README.md` and `README_RU.md`, 4.12.

