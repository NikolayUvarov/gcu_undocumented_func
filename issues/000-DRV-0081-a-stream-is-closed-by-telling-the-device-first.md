# 000-DRV-0081 — `usb_host`: a stream is closed by telling the device first, then ending its packets

**Type:** driver (`usb_host`, `audio_gw`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** in progress (2026-11-22: written on `claude/000-DRV-0081-a-close-tells-the-device-first`, from 000-DRV-0101's landing head; no hardware run: the speakerphone is not on the Ryzen) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-12.1

## Problem

**On the Ryzen, 2026-11-21,** the STM-Speaker's own counter of playback frames it lacked rose at the close of every stream from MIND Core, and only then:
- 92 after a stream of five minutes, counted from zero;
- still 92 through the minute of notes that followed, watched by the maintainer;
- 174 once that minute's stream was closed.

Before that the board had counted 548 over nine streams. No stream lost a packet on the host's side.

**Its size varies.** Two more closes that day added 33 and 45 frames (recorded on `claude/DRV-ryzen-peripherals`, 0ef9e89b0). Every close read so far added some: 92, 82, 33 and 45.

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

## Progress (2026-11-22)

**What is written** (`usb_host/src/close.rs`, `main.rs`, `xhci.rs`):
- **The order.** A `select` to a setting without a stream, while the interface streams, is a close (`close::order`). `Controller::close_streams`:
  1. marks the interface's streams closing;
  2. sends SET_INTERFACE while their TDs still go out;
  3. then drops their endpoints in one Configure Endpoint, and frees their rings and buffers.

  The endpoints are dropped whatever the request's end. A client that releases its interface, or whose new instance takes its place, closes its streams the same way.
- **The close's errors.** From the mark on, a TD that ends in error is not counted in `WITH ERRORS` (`close::counted`). It is said on a line of its own after the stream's last line: `n TDS ENDED IN ITS CLOSE, THE DEVICE TOLD (COMPLETION c)`. A dry ring and a missed service count as always: the device saw them.
- **What the close took**, one line each: `STREAMS CLOSED, THE DEVICE TOLD FIRST: SET_INTERFACE DONE IN x MS, ENDPOINTS DROPPED IN y MS`. That is for the hardware run.
- **No descriptor read for setting 0.** A `select` to setting 0 reads no configuration descriptor when the interface's setting-0 endpoints, kept from its set-up, have no isochronous one (`close::reads_descriptor`). That is two control transfers fewer in every close, the speakerphone's and a bulk camera's stop step (`video_gw`'s `select` to 0) alike.

**Not Linux's order.** Linux's `usb_set_interface` (`drivers/usb/core/message.c`, v6.12) does it the other way round:
- it flushes the interface's endpoints;
- it drops them and adds the new ones (`usb_hcd_alloc_bandwidth`);
- only then does it send SET_INTERFACE.

That is the order `usb_host` had. Whether the board counts lacking frames at a close under Linux too has not been read.

**What this does not cover.** A duplex's capture close holds `audio_gw` while `select` runs. 000-DRV-0094's step 1 run saw feedback and playback dry for about 38 ms at it; run D saw playback dry for 12.5 ms, `NOT WAITING`.
- The new order does not shorten that close.
- The skipped descriptor read shortens it by two control transfers.
- The close line will say on the Ryzen how long each part takes.

**Tests:**
- **Host:** `tests/usb_close_host.rs`: the order for each case, when a select reads the descriptor, and what each completion code counts as, closing or not.
- **QEMU:** the `usbaudio` suite checks that beep's close told the device first, and that the stream's last line has 0 `WITH ERRORS`. Its first run is to come.

## Related

[000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md) (USB audio; its last criterion waits for this).
