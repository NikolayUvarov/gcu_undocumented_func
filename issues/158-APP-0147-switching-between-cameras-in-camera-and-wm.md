# 158-APP-0147 — Switching between cameras in `camera` and in `wm`

**Type:** tools (`camera`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — to build against the test pattern; the Ryzen's two USB cameras need DRV's [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) · **Main task:** [158](158-video-capture.md) · **Roadmap:** track G · **Constitution:** MC-3.1, MC-12.1

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

## Related

[158](158-video-capture.md), [158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md), [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) (DRV's branch `claude/DRV-ryzen-peripherals`).
