# 158-APP-0152 — A key in `camera` and `wm`'s camera that steps through the camera's modes

**Type:** tools (`camera`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** `DRV`: [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) (`video.wit` 1.1's `modes`) · **Main task:** [158](158-video-capture.md) · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.4

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
