# 158-APP-0005 — The camera mark was missing once on CI

**Type:** tools (test) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** the next failure on CI · **Main task:** [158](158-video-capture.md) · **Roadmap:** tracks A, G · **Constitution:** MC-11.4, MC-12.2

## Problem

The camera mark tells the user that a camera stream is open (issue 158, MC-11.4). `camera_check` in `tests/qemu_smoke.py` opens a stream with `camera -r 10 -t 3`, waits 1 s after `[CAMERA] OPENED` and reads a pixel of the mark from a screendump. On CI it failed once, on the branch `claude/wizardly-franklin-kec1a9` at 86753e9, in the group "QEMU (NVMe boot disk)" (`--disk nvme --suites vfs`, 4 CPUs, TCG):

```
AssertionError: ('the camera mark while the stream is open', b'\x00\x00\x00')
```

The guest log ends at `[PID 25] [CAMERA] OPENED`. Every other group passed on the same commit, `camera_check` included.

It did not happen locally with the same QEMU (8.2.2) and the same arguments:

- 3 runs of the group, and 5 more with every host CPU kept busy, all passed;
- a run that sampled the pixel every 0.63 s saw the mark lit through the whole stream and gone after it (`1e1e2e`, the background).

Reading the code found no path that leaves the mark out:

- `video_gw` calls `display::camera` when the stream opens and every 500 ms while it is open (`MARK_MS`), and ignores a failed call;
- the compositor keeps the mark for 1.5 s after each call (`DOT_MS`) and draws it over the screen in front.

A black pixel (`000000`) is neither the mark nor the background, so the screendump may have been taken while the screen was being drawn, or before the camera's screen was shown at all.

## Plan

1. The check keeps failing in that case, and says more when it does (done with this issue): four more samples of the pixel 0.3 s apart with their time since `OPENED`, the camera program's own output to its end, and the gateway's log.
2. With that output from the next failure, find the cause: a mark that comes late, one that blinks, or a screen not drawn yet. Fix it in the program, the service or the check, whichever is wrong.
3. If it happens no more in a month of CI runs, close the issue with what is known.

## Acceptance criteria

- The cause is known, and fixed where it lies, or the issue is closed under step 3.
- The check is not weakened: it still fails when the mark is missing while a stream is open.

## Related

[158](158-video-capture.md), [164](../issues-done/164-usb-hid-keyboard-and-mouse.done) (the recording dot of the compositor).
