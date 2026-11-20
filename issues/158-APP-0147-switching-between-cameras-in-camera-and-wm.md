# 158-APP-0147 — Switching between cameras in `camera` and in `wm`

**Type:** tools (`camera`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done in QEMU against the test pattern, 2026-11-20; the Ryzen's two cameras left) · **Blocked by:** — to build against the test pattern; the Ryzen's two USB cameras need DRV's [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) · **Main task:** [158](158-video-capture.md) · **Roadmap:** track G · **Constitution:** MC-3.1, MC-12.1

**Asked by:** DRV, `APP/2026-11-19T2136Z-DRV-request-switching-between-cameras-in-camera-and-wm.md` on `internal-dev-messages`, at the maintainer's word of 2026-11-20: «надо уметь переключаться между камерами». The Ryzen has two USB cameras. Numbered on 2026-11-20.

## Problem

- **`camera` and `wm`'s camera open the first camera alone.**
- **`video_gw` lists every camera** through `cameras`: the test pattern now, and with 158-DRV-0072 up to 4 USB cameras before it. `open(n)` on another camera closes the owner's earlier stream first.

## Plan

- **`camera`:**
  - `-c <n>` picks a camera, 0 by default;
  - a key switches to the next one while it runs. The key is `n`, and Tab too.
  - The title names the camera by the name `cameras` gives, with its number and the count: `camera 2/3: <name>`.
- **`wm`'s camera:** the same choice, from a list of the names `cameras` gives, and the same key.
- **A camera that cannot open** is named, and the earlier one stays when it can.

## Acceptance criteria

- **Host tests:** the next camera's number, wrapping round; `-c` out of range refused with the count.
- **QEMU:** `camera -c 1` and the switch key, against the test pattern and the cameras QEMU's suite gives.
- **On the Ryzen,** with DRV, both USB cameras one after the other. This is evidence for that machine (MC-12.1).

## Progress (2026-11-20)

- **The code** (5ba5c0f8; `fast-test` 11f41a58):
  - `camera/src/pick.rs`: the camera `-c` names, checked against the count, and the next one round;
  - `camera/src/main.rs`:
    - `-c <n>`;
    - `n` or Tab while it shows, which closes the stream, opens the next camera at the same size and rate, and opens the one before again if the next does not;
    - `[CAMERA] SWITCHED TO <name> (2/3)`, and `[CAMERA] ONE CAMERA ONLY: <name>`;
    - the window's title `camera 2/3: <name>`.
  - A still or a recording keeps its camera.
- **`wm`'s camera** is the `camera` program in a window: the title names the camera, and the same key switches it. A list of the names in `wm`'s own menu is not made. It needs `wm` to ask the gateway before it starts the program, and the window's key does what the request names.
- **Tested** in this container, x86, 4 CPUs:
  - `tests/camera_host.rs`;
  - the `vfs` suite's `camera_check`: `-c 1` refused with the count, `n` with one camera named, Esc;
  - the `wm` suite's camera window: `vfs` and `wm`, 25 PASS.
- **Left:** the Ryzen's two USB cameras with DRV, once 158-DRV-0072 is in `main`. The gateway lists the test pattern alone in QEMU, so a real switch between two streams has not run.

## Related

[158](158-video-capture.md), [158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md), [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) (DRV's branch `claude/DRV-ryzen-peripherals`).
