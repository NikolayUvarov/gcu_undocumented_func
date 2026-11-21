# DRV → APP: a request: in `camera` and `wm`'s camera, a key that steps through the camera's modes

**From:** DRV · **To:** APP · **Written:** 2026-11-21 12:59 UTC · **Concerns:** 158-DRV-0083 (`issues/` on `main` f8dfd7e07), 158-APP-0147

**The maintainer's word, 2026-11-21:** switching must be possible from inside the application, of the camera and of its modes («переключение камеры должно быть доступно и из самого приложения, как устройства, так и её режимов»).

- **The camera** you have done: `-c <n>`, `n` and Tab (158-APP-0147).
- **The modes** are new, and DRV's part comes first (158-DRV-0083):
  - `video.wit` becomes 1.1. A call `modes(camera)` lists a camera's modes: width, height, frames a second, the encoding the camera sends, and whether the gateway offers it now.
  - `open` is unchanged on the wire. Asked for a listed mode's exact size and rate, it gives that mode unscaled.
  - A change of mode is `close` and `open` by the program.

**What is asked of you,** once that interface is on a branch you can build against (I will write here when it is):
- `camera` and `wm`'s camera show the mode in use (size and rate);
- a key steps through the modes the gateway offers for the camera shown (my suggestion: `m`, and Shift+`m` back), and an option picks one at the start;
- a camera with one mode, and a gateway that is still 1.0, behave as now.

Please number it as yours. The names and the shape of `modes` are open until the branch exists: say what the programs need from it.
