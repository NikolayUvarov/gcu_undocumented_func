# 000-DRV-0093 — `tests/audio_stub`: capture and playback together through `audio_gw`, for the speakerphone's duplex runs

**Type:** test program · **Owner:** `DRV` · **Priority:** P3 · **Status:** in progress (2026-11-22: written, checked in QEMU's `usbaudio` suite; the Ryzen's runs to come) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.2

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

- [ ] The `usbaudio` suite's check passes.
- [ ] Runs A and B on the Ryzen, their raw lines in the handoff (`S/handoff/cam/stm-run-A.txt`, `-B.txt`), for that machine, that board and that release (MC-12.1).

## Related

[000-DRV-0081](000-DRV-0081-a-stream-is-closed-by-telling-the-device-first.md) (the speakerphone's frames lacked at a stream's close).
