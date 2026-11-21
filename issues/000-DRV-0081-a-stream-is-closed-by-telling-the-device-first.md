# 000-DRV-0081 — `usb_host`: a stream is closed by telling the device first, then ending its packets

**Type:** driver (`usb_host`, `audio_gw`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** open (2026-11-21) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-12.1

## Problem

**On the Ryzen, 2026-11-21,** the STM-Speaker's own counter of playback frames it lacked rose at the close of every stream from MIND Core, and only then:
- 92 after a stream of five minutes, counted from zero;
- still 92 through the minute of notes that followed, watched by the maintainer;
- 174 once that minute's stream was closed.

Before that the board had counted 548 over nine streams. No stream lost a packet on the host's side.

**Its size varies.** Two more closes that day added 33 and 45 frames. Every close read so far added some: 92, 82, 33 and 45.

**Why.** When nothing is left to play, `audio_gw` sends a second of silence, gives no more packets and selects setting 0. `usb_host`'s `select` then:
1. reads the configuration descriptor;
2. drops the isochronous endpoints from the controller, so their packets end;
3. sends SET_INTERFACE 0.

Between 2 and 3 the device still holds its stream open and gets nothing. The speakerphone counts the block it could not fill, 82 to 92 frames. The interval itself was not measured; this is read from the order of the code and from the board's `sp_usb_stream_playback_read`.

It is under 2 ms of silence after a second of silence, so nothing is heard. But a device's own count of faults should stay at zero when nothing went wrong.

## Plan

- **`select` to a setting without streams** sends SET_INTERFACE first, while the packets queued still go out, and drops the endpoints after it.
- **The stream's last transfers are not counted as errors.** The feedback endpoint's transfers that the device no longer answers after SET_INTERFACE 0 must not show as `WITH ERRORS` in the stream's last line: the stream is marked closing before the request.
- **The ring and its buffers stay** until the endpoints are dropped.

## Acceptance criteria

- [ ] The `usbaudio` suite passes, and its stream's last line shows no error from the close.
- [ ] On the Ryzen or the DEXP: the speakerphone's `USB P нед/пер` is 0/0 after a five-minute stream from zero, read after the stream was closed.

## Related

[000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md) (USB audio; its last criterion waits for this).
