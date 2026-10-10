# 158-APP-0043 — The camera from `wm` and `console`

**Type:** tools (`wm`, `console`, `shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer's run on the MacBook Pro, 2026-10-10) · **Status:** in progress (done in QEMU; waits for the maintainer's run on the MacBook Pro) · **Blocked by:** — · **Main task:** [158](158-video-capture.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

Numbered from the kernel track's request in `requests-APP.md` ("The camera from `wm`, and the shell in a window", 2026-10-10, at the maintainer's request after the run of `fast-test` bc681376b8d8 on the MacBook Pro). The request's other parts are [211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done) (the shell in a window) and [211-APP-0044](211-APP-0044-console-joined-to-the-shell.md) (`console` joined to the shell).

## Problem

`camera` streams the MacBook Pro's FaceTime HD camera from the shell's screen: 64 frames at 320×240, none broken (`log:boot0001.log`). Started from `wm`'s menu or from `console`, it ends with `camera: no camera was granted (start camera from the shell and allow it)`:

- `wm` does not ask the shell for the camera (its `mind::request!` has no `REQUEST_CAMERA`), so it holds nothing in `SLOT_CAMERA`;
- `wm`'s `start()` and `console`'s `run()` do not handle `REQUEST_CAMERA`, so a program they start never gets it.

## Plan

- `wm` asks for `REQUEST_CAMERA`. The shell lends it without a question, by the maintainer's rule of 2026-10-09: starting a program whose purpose is the camera is the user's request; the camera mark shows while a stream is open.
- `wm` lends `SLOT_CAMERA` to a program that asks for it and names it in its `STARTED … WITH` line. `console` asks for it too and lends it to what it starts.
- A program started from the shell's window (211-APP-0040) gets the camera from the shell, as on the shell's screen.
- The `wm` suite's check of a program that ends at once with a failure (211-APP-0039) takes another program than `camera`, which now has a camera.
- **Docs:** `docs/tools` (EN, RU).

## Acceptance criteria

- The `wm` suite starts `camera` from the run line and from `console` with the video gateway's synthetic source, and sees its window show the stream (the test pattern's colours in its frame).
- The shell's window starts `camera`, and its window shows the stream.
- On the MacBook Pro (the maintainer's run): `camera` shows the FaceTime camera in a `wm` window.

## Progress (2026-10-10)

- **Done:** `wm` asks for `REQUEST_CAMERA` and lends `SLOT_CAMERA` to a program it starts that asks for it (`[WM] STARTED camera PID n WITH window,files,camera`); `console` asks for it and lends it too; the shell's window lends it as the shell's screen does (211-APP-0040).
- **Checked in QEMU** (the `wm` suite, x86, on the branch): the boot disk carries `video/synthetic`; `camera` started from `wm`'s run line, typed in a `console` window and typed in the shell's window each shows the test pattern's yellow, cyan and magenta bars in its window, and closed, its stream ends. 211-APP-0039's check now starts `camera -z 100x100`, a size the source does not give: its window keeps `camera: cannot open …` and `ENDED (STATUS 1): PRESS A KEY` until a key.
- **Docs:** `docs/tools` (EN, RU), §4.9, §4.10 and §4.12.
- **Left:** the maintainer's run: `camera` in a `wm` window on the MacBook Pro's FaceTime camera (in `fast-test` from bb240f7).

## Related

[158](158-video-capture.md), [211-APP-0039](../issues-done/211-APP-0039-an-ended-program-leaves-its-message-in-its-window.done), [211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done), [211-APP-0044](211-APP-0044-console-joined-to-the-shell.md).
