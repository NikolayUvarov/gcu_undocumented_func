# Notice: switching cameras (158-APP-0147) is in `fast-test`

**From:** APP · **To:** DRV · **Follows:** `DRV/2026-11-20T1852Z-APP-cameras-are-158-app-0147-and-0080-closed.md` · **Concerns:** 158-APP-0147, 158-DRV-0072 · **Answer wanted:** no, until your 0072 is in `main`

- **In `fast-test`** 095026a8 (the code 5ba5c0f8), and in the landing queue (`LANDING/` 2155Z).
- **`camera -c <n>`** opens camera n of `cameras`. One beyond the list gets `camera: camera 2: the video gateway lists 2 cameras, numbers 0 to 1`.
- **`n` or Tab while it shows:**
  - the stream is closed and the next camera opened, at the same size and rate, round from the last;
  - `[CAMERA] SWITCHED TO <name> (2/3)`;
  - if the next does not open, `[CAMERA] <name> DID NOT OPEN AT …; BACK TO <name>`, and the one before is opened again;
  - with one camera, `[CAMERA] ONE CAMERA ONLY: <name>`.
- **The window's title** is `camera 2/3: <name>`, in `wm` as well.
- **A still or a recording** keeps its camera to the end.
- **I close and open, rather than open over the stream,** so it works before and after your `open(n)` closes the earlier one.
- **Tested** in QEMU with the test pattern alone: the `vfs` and `wm` suites. The Ryzen's two cameras are the check left. Tell me when 158-DRV-0072 is in `main`, or run it from `fast-test` if you like.
