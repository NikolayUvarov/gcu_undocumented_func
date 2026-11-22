# 000-DRV-0093 — `tests/audio_stub`: capture and playback together through `audio_gw`, for the speakerphone's duplex runs

**Type:** test program · **Owner:** `DRV` · **Priority:** P3 · **Status:** in progress (2026-11-22: written, checked in QEMU's `usbaudio` suite, run A made on the Ryzen; run B not wanted now) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.2

## Problem

The STM-Speaker project's agent asks for two 15-minute duplex runs of its USB audio board on the Ryzen: capture and playback open together, with the host's per-endpoint counters at the end. Nothing in the tree keeps both open that long:
- `listen` records 10 s at most, and then plays it back;
- `listen --vad` captures alone, 60 s at most;
- `beep` plays at most 64 notes of 10 s.

## Plan

1. **A test program like `tests/video_stub`:** `audiostub duplex SECONDS [HZ]`.
   - It opens capture and plays a tone of HZ (0: silence, its playback stream kept open) through `audio_gw`, for SECONDS.
   - Neither call waits on the other: a full playback queue or an empty capture passes the turn.
   - At the end it says the capture's buffers, frames and overflows, the frames played and how often the queue was full, the start and end by the machine's uptime, and after 3 s the hardware tests' marker `*** AUDIO DUPLEX Complete. Data collected ***`.
2. **A check in QEMU's `usbaudio` suite.** QEMU's `usb-audio` has no capture: the stub says so, plays 3 s of tone and ends with the marker.
3. **On the Ryzen:** run A with the board behind the Genesys hub, then run B on a root port, `audiostub duplex 900 0`, with `usb_host`'s `STREAM ENDED` lines for DCI 2, 3 and 5 and `audio_gw`'s lines kept.

## Acceptance criteria

- [x] The `usbaudio` suite's check passes. (2026-11-22, LEG9: no capture on QEMU's usb-audio, said; 176 640 frames of a 440 Hz tone in 3002 ms; the marker line.)
- [ ] Runs A and B on the Ryzen, their raw lines in the handoff (`S/handoff/cam/stm-run-A.txt`, `-B.txt`), for that machine, that board and that release (MC-12.1).

## Progress (2026-11-22)

- **The stub and its check:** aba27bb57. The `usbaudio` suite passed on LEG9 with its `audiostub` check.
- **On the Ryzen:**
  - it is started by the cable's `run` with the file's own name (`run name=audiostub.elf args="duplex 900 0"`), after `put` into `data/link/`;
  - the shell refuses `run … &` typed from the link;
  - a 1 s trial gave capture 46 buffers with 0 overflows, and playback.
- **Run A, 2026-11-22 06:54:47–07:09:50 UTC** (release 202611220644; the STM-Speaker board `CAFE:4014`, full speed, behind the Genesys hub at XHCI 1 port 1.4; `audiostub duplex 900 0`, silence; the raw lines in `S/handoff/cam/stm-run-A.txt`):
  - **Playback** (DCI 2, endpoint 01): 901 612 packets sent, 0 missed, 0 times dry, 0 errors. `audio_gw`: 901 625 packets, 1025 of them silence, the last rate 47.989 frames a ms.
  - **Capture** (DCI 3, endpoint 81): 86 371 174 bytes, 0 missed, 6 times dry, 0 errors. The stub read 42 115 buffers, 43 184 128 frames (47 979 a second of 48 000), 0 overflows.
  - **Feedback** (DCI 5, endpoint 82): 2 704 317 bytes, 0 missed, 6 times dry, 0 errors.
  - **The dry rings:** the first of each IN stream came 46 and 47 s into the run, within a few seconds of a 155 KB log read over the cable (the CH340 on the same hub, served by the same `usb_host`). The other five of each are not logged by time, since `usb_host` names a completion code once a stream.
  - **What it shows:** 15 minutes of both directions on the AMD xHCI through a full-speed device behind a high-speed hub, with neither stream ended and no packet missed on the wire. Each dry ring is a gap of 8 ms or more with no IN token, on our side. It holds for that machine, that board and that release (MC-12.1).
- **Run B (a root port)** is not wanted now: the STM-Speaker agent found its board's restarts in its own debugger's reads of the board's registers (the owner session, 2026-11-22).

## Related

[000-DRV-0081](000-DRV-0081-a-stream-is-closed-by-telling-the-device-first.md) (the speakerphone's frames lacked at a stream's close).
