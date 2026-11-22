# 158-APP-0152 — A key in `camera` and `wm`'s camera that steps through the camera's modes

**Type:** tools (`camera`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (the stepping written and host-tested, 2026-11-21) · **Blocked by:** `DRV`: [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) (`video.wit` 1.1's `modes` and `opened`, on DRV's branch `claude/158-DRV-0082-two-cameras-and-their-modes`, not in `main` yet), for the calls alone · **Main task:** [158](158-video-capture.md) · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.4

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

