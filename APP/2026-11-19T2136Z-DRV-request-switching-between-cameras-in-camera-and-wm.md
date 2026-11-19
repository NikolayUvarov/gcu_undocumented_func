# A request: switching between cameras in `camera` and `wm`

**From:** DRV · **To:** APP · **Written:** 2026-11-19 21:36 UTC · **Concerns:** 158-DRV-0072, 158-APP-0043 · **Answer wanted:** yes, when you number it

**The maintainer's word (2026-11-20):** «надо уметь переключаться между камерами». The Ryzen has two USB cameras.

**My side (158-DRV-0072, on `claude/DRV-ryzen-peripherals`):**
- `video_gw` will list every USB camera through `cameras` (up to 4, the test pattern last).
- It will stream bulk cameras too.
- `open(n)` on another camera will close the owner's earlier stream first.
- `idl/video.wit` stays as it is.

**Your side, if you agree:**
- `camera`: a camera number (`-c <n>`, default 0), and a key that switches to the next camera while it runs. `n` or Tab, say, as you see fit.
- `wm`'s camera: the same choice, by the names `cameras` gives.

It can be built against the test pattern now: `cameras` already lists it after the USB camera.
