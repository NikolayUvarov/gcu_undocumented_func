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

## Progress

**Second failure, with the diagnostics** (CI, dc914bc, the same group, 2026-10-07):

- The pixel stayed `000000` at 1.0, 1.9, 2.7, 3.5 and 4.3 s after `OPENED`: the mark never came; it did not come late or blink. Black is not the camera program's background either (`1e1e2e` in a local run).
- The program wrote `30 FRAMES (4 PICTURES) … SEQUENCE 1..30`: it read every frame on time and encoded 4 of 30 pictures (9 locally).
- `video_gw` logged `CLOSED AFTER 604 FRAMES` (34 locally): the stream stayed open about 60 s, not about 3.5. `ps` afterwards: `video_gw` had run 39 times in all; `vfs_server` 28 149, `ramdisk` 15 760, `nvme` 9 487, `rtc` 1 447.
- So the program spent about a minute after its last frame, most likely writing the file; meanwhile `video_gw` hardly ran. Yet the heartbeat sent at the opening alone should have lit the mark for 1.5 s, and it was dark at 1.0 s. Either the compositor did not run in those seconds, or its calls failed.

**Third failure** (CI, 3c943da, group "files and block writes", the ATA disk): the mark was there this time, but the recording of 3 s did not end within 60 s; the guest log ends at `OPENED`. The suite had taken under a minute until then. So the cause is not NVMe: a camera run on CI sometimes stalls for a minute or more, mark and all, while every earlier step of the suite runs at its usual pace. On the branch it happened in three of four CI runs; `main`'s last four CI runs passed the same check.

The check now also reports how much of the screenshot is black. On a stall, missing mark or a recording that does not end, it presses Ctrl+Z, which gives the shell back while the program runs on, and reports `ps` and `stat` of every task on the camera's path: what each one waits for during the stall.

**Fourth failure, with the stall report** (CI, 82bb9c6, "files and block writes", 2026-10-07; `main`'s CI on the same code, 73689c1, passed). Taken 5.8 s after the program started (`AGE_MS=5834`):

- The mark pixel black at 1.0–4.4 s, and 93% of the screen black: the camera's own screen, a 320×240 preview on black, so the compositor was compositing it. The compositor was in its receive (`RECV`), alive, 2 704 runs since boot.
- `camera`: `STATE=CALL WAIT=14`, waiting on `video_gw` for a frame; 61 ms of CPU.
- `video_gw`: `STATE=SLEEP`, 36 runs and 31 ms of CPU since boot, 18 messages received, 11 sent. It sleeps in `Stream::next` until a frame is due; at about 14 requests in 5.8 s it gave a frame every ~800 ms instead of every 100 ms.
- The clocks agree: `logd`'s `AGE_MS` (52.7 s, TSC-based) matches the job's wall clock, so the TSC calibration is not off.
- `ata` (on `video_gw`'s CPU 1) had done 503 265 system calls, its port I/O, and was idle at the moment of the report.

Two things are wrong and not yet explained: `video_gw` sleeps far longer than the frame period, and no heartbeat lit the mark although `video_gw` sent some. Both are in `video_gw` and the compositor, not in the tools track's files: `video_gw` belongs to the drivers track (`DRV`, open, no owner), and the compositor to none listed in TRACKS.md. Logging there (a heartbeat the compositor refused; the mark switched on and off) would tell the rest.

## Plan

1. The check keeps failing in that case, and says more when it does (done with this issue): four more samples of the pixel 0.3 s apart with their time since `OPENED`, the camera program's own output to its end, and the gateway's log.
2. With that output from the next failure, find the cause: a mark that comes late, one that blinks, or a screen not drawn yet. Fix it in the program, the service or the check, whichever is wrong.
3. If it happens no more in a month of CI runs, close the issue with what is known.

## Acceptance criteria

- The cause is known, and fixed where it lies, or the issue is closed under step 3.
- The check is not weakened: it still fails when the mark is missing while a stream is open.

## Related

[158](158-video-capture.md), [164](../issues-done/164-usb-hid-keyboard-and-mouse.done) (the recording dot of the compositor).
