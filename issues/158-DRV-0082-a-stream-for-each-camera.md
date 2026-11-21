# 158-DRV-0082 — `video_gw`: a stream for each camera, so two cameras show at once

**Type:** driver (`video_gw`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-21: a stream for each camera, and `usb_host` keeping each bulk camera polled (`usb.wit` 1.6); the two cameras streamed at once on the Ryzen to two test programs; the maintainer's look at two windows and the gate wait) · **Blocked by:** — · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-11.4, MC-12.1

## Problem

`video_gw` holds one stream. It lists every camera ([158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md)), but a second program that opens another camera is refused while the first streams. The maintainer asked for both ways at once (2026-11-21): two windows with a live picture each, **and** one program that switches between the cameras. The Ryzen has two USB cameras (`0471:A6CA` and `0471:A6CD`), each straight in a root port of its own xHCI.

`idl/video.wit` already says that a camera has one owner at a time. So the interface does not change: the gateway's one-stream limit goes.

## What exists

- **The gateway's change,** written on 2026-11-21 and not committed:
  - a stream for each camera, keyed by the camera, one owner each; a second owner of a camera that is held gets `busy`;
  - while one reader waits for its frame, the other cameras' transfers are pumped;
  - `video/synthetic` beginning with `2` gives two test patterns.
- **A QEMU suite `cameras`,** written beside it: two test patterns listed, one recorded for 8 s (80 frames) while a still is taken of the other, and the camera a program holds is busy for another. It passed on the bench tree, which had APP's `camera -c` merged.
- **On the Ryzen** that gateway is in release 202611211156. Nobody has looked at its picture yet.

## Plan

1. Commit the gateway's change with its host cases.
2. Add the `cameras` suite to `tests/qemu_smoke.py`, `scripts/ci_local.sh` and `.github/workflows/ci.yml`, once `camera -c` is in `main`.
3. On the Ryzen, with both cameras:
   - two `camera -c <n>` windows in `wm`, both live, for five minutes; `video_gw`'s frame counts for each;
   - one `camera` that switches (`n` or Tab) while a second program holds neither camera, and while it holds one;
   - what each camera's stream costs its bus (isochronous or bulk, the setting chosen), from the gateway's lines.
4. Update `README.md` and `docs/api` where they say one stream.

## Acceptance criteria

- [ ] `cameras` runs in the gate and passes: two cameras streaming to two programs at once; a held camera is `busy` for another program.
- [ ] On the Ryzen two windows show the two cameras at once for five minutes, with the frame counts recorded; the maintainer saw both pictures move. This holds for that machine and those cameras (MC-12.1).
- [ ] One program switches between the cameras while the other window stays live.
- [x] `idl/video.wit` is unchanged by this task (its 1.1 is 158-DRV-0083's; `usb.wit` 1.6 is this task's, for the bulk streams).

## Progress (2026-11-21)

**The gateway's change, reviewed and committed** on `claude/158-DRV-0082-two-cameras-and-their-modes`:
- **The choices that need no device** are in `video_gw/src/choices.rs`: which camera a number names, whether a program may open it beside the streams held, how many patterns `video/synthetic` asks for. Host cases: `tests/video_gw_host.rs` (6).
- **Fixed against the bench diff:**
  - a stream whose camera went is marked so: it takes no more packets, holds its camera for nobody, and is closed without asking the device;
  - a pattern's reader waits for its frame while the cameras' streams are pumped, rather than in one sleep of up to a second.

**Two bulk cameras at once on the Ryzen failed** with the bench diff (release 202611211156, the maintainer at the keyboard, 2026-11-21, log `S/rz-cam-user1.txt`):
- **One program switching** between the cameras worked: for example camera 1 for 61 s, `618 FRAMES`, the camera's `1756 GOOD FRAMES, 63 BROKEN`.
- **Two programs at once did not.** Camera 0 streamed; camera 1, opened beside it, got `NO GOOD FRAME FOR 5000 MS: 0 GOOD FRAMES, 2 BROKEN (THE LAST 65524 BYTES OF 153600), 12 PACKETS` twice, and camera 0's picture jerked.
- **Why, as the code reads:**
  - every bulk read is one blocking call to `usb_host`, which queues one transfer and waits for it;
  - the camera streams from its commit on at its sensor's pace, and `pump_bulk` read a camera until a whole frame ended (about 33 ms);
  - meanwhile the other camera's endpoint had no transfer, so nothing was read from it and its buffer overflowed; its frames came cut (65524 bytes, one payload of three).
- **The change:** while two bulk cameras stream, a pump is one transfer of 16 KiB at most, the cameras in turn, and the loop waits 1 ms at most for a request. Each camera then waits one 16 KiB transfer of the other's, not a frame. The close line of a bulk stream now gives its pumps and its transfers' time (`… PUMPS, … TRANSFERS IN … MS OF … MS OPEN, THE LONGEST … MS`), to see where the time goes.
- **Run on the Ryzen, 2026-11-21 15:04-15:07 UTC: it does not cure it.** The gateway was 164518a43's, put in with `replace`. Both cameras were read by `tests/video_stub` from `data/link/`, started from the console's shell, 320×240 YUY2 at 10 a second:
  - **camera 1 alone:** 10 s, 226 good frames and 32 broken; `1000 TRANSFERS IN 9976 MS OF 10038 MS OPEN, THE LONGEST 249 MS`. Each transfer waits for the camera's bytes, about 10 ms on average, so the gateway spends almost all its time inside transfers;
  - **both, in 16 KiB turns:** camera 0 opened, then camera 1 beside it 9 s later. Camera 1 gave no good frame in 5 s: `10 PUMPS, 10 TRANSFERS IN 1883 MS OF 5109 MS OPEN, THE LONGEST 216 MS`. Camera 0 lost its frames too from then on: `NO GOOD FRAME FOR 5000 MS`, then closed with 133 good and 14 broken, `593 TRANSFERS IN 13280 MS OF 15685 MS OPEN, THE LONGEST 417 MS`;
  - **both, in 4 KiB turns** (a test build): the same. Camera 1 had `10 TRANSFERS IN 1846 MS`, 3.9 KiB each, the longest 207 MS; camera 0's longest transfer was 598 MS.
- **What this shows:**
  - once the two streams run together, a single transfer of a few KiB takes about 200 ms, against 10 ms alone, whatever its size;
  - so the cameras do not merely overflow between short turns: each stalls while the other's transfer holds the gateway (and `usb_host`, which waits for each transfer before it answers), and then starts again late.
- **The cure:** keep transfers queued on both endpoints at once, as Linux's `uvcvideo` keeps several URBs per stream. `usb_host` then reads both cameras while the gateway does other work: a bulk IN stream, through a new call of `usb.wit` (1.6), drained as an isochronous one is. That is the next commit.
- **The bulk IN stream** (`usb.wit` 1.6, `usb_host/src/xhci.rs`):
  - **`bulk-stream(handle, address, on)`** keeps a claimed interface's bulk IN endpoint polled on xHCI. Up to 32 transfers of a page (`mind::usb::BULK_STREAM_BYTES`) stay queued on the endpoint's own ring, as many as leave 12 pages of the controller's pool free, 8 at least.
  - **`isochronous` gives them** in order: an empty one too, since it ends a payload; bit 15 for one that ended in error, after which the endpoint is reset on both sides and polled again.
  - **`bulk` is refused** on the endpoint while the stream runs. The stream ends with `on` false, with the interface's release, or with the slot; the endpoint's ring then goes back to it.
  - **EHCI keeps no such stream** (invalid).
- **The gateway with it:** a bulk camera's endpoint is kept polled from its commit to its close, and its transfers are taken without waiting. Where usb_host refuses (EHCI), the gateway reads by waiting transfers as before and says so. The turn-taking of 16 KiB is gone. A stream's close line now gives its pumps and the longest gap between two (`… PUMPS IN … MS, THE LONGEST GAP … MS, BULK KEPT POLLED BY USB_HOST`): usb_host's queue for the camera must outlast that gap.
- **Run on the Ryzen, 2026-11-21 15:23-15:51 UTC: the two cameras stream at once.** `usb_host` (13ea6031d) and `video_gw` were put in with `replace` (until the next boot). `usb_host` came back with every device, and `usb_storage` started afresh with the same medium. `tests/video_stub` read the cameras from the console's shell:
  - **320×240 YUY2 at 10 a second each**, camera 0 for 40 s and camera 1 for 30 s beside it (the gateway of ab90e40b2):
    - camera 0: `401 FRAMES`, the camera's `664 GOOD FRAMES, 0 BROKEN`;
    - camera 1: `300 FRAMES`, `684 GOOD FRAMES, 127 BROKEN`;
    - `KEPT POLLED BY 32 TRANSFERS OF 4096 BYTES` on each controller; `0 WITH ERRORS`; the longest gap between two pumps 63 and 78 ms;
    - alone in the morning, camera 1 had broken 32 of 258 frames.
  - **640×480 each, 20 s** (c687acf56's gateway):
    - camera 0 at 25 a second: `499 FRAMES`, `331 GOOD FRAMES, 0 BROKEN`, longest gap 10 ms;
    - camera 1 at 30 a second: `599 FRAMES`, `569 GOOD FRAMES, 3 BROKEN`.
  - The figures hold for that machine and those cameras (MC-12.1).
- **What is left:**
  - **The longest gaps come from the gateway holding itself:** 283 to 475 ms, while the other camera's stream closed or changed its mode. Frames of the camera beside it break meanwhile. An earlier line here put this down to the reset itself, which was wrong. The logs give it to two things:
    - **my own waits:** 250 ms after each close (c687acf56) or before each re-open (b712571ac);
    - **the camera's late answer:** after its reset (setting 0 selected again), camera 0 answered the next probe late. From close to the probe's answer took 412 ms with no wait (`2211.670 CLOSED` to `2212.082 ASKED`) and 404 ms after the 250 ms wait.
    - The clear-halt and the reset together took 32 to 35 ms (from `BULK STREAM ENDED` to the close's line).
  - **The maintainer has not looked at two windows yet.** The machine runs these builds until its next boot for that.
- **The reset at close off the gateway's main path** (the owner session's step 2):
  - **At a bulk camera's close** only its stream in `usb_host` ends (`bulk-stream` off: two controller commands). The closing program then has its answer.
  - **The clear-halt and the reset come after it,** one a turn of the gateway's loop, every stream pumped between (`Usb::step`).
  - **An open of the camera that comes sooner** finishes those steps and then waits out its settling while it pumps every other stream (`settle`), instead of sleeping or sending a probe the camera answers late.
  - **The settling** starts at 250 ms after the reset. A probe and commit that still take more than 20 ms lengthen it by that time, up to 1 s (`choices::Settle`, `learned`). Camera 0's figures above would make it 400 ms after one change.
  - **The other readers' calls** still wait while the gateway settles a camera. Their streams keep being read meanwhile, so their next frame is whole, only late.
  - **The log says** `STOPPED n MS AFTER ITS CLOSE: CLEAR-HALT a MS, SETTING 0 b MS, A STEP A TURN` and, at a re-open, `OPENED AGAIN n MS AFTER ITS RESET, w MS WAITED (THE OTHER STREAMS PUMPED); ITS PROBE AND COMMIT p MS; IT SETTLES s MS NEXT`.
  - **Tested:** host cases (`tests/video_gw_host.rs`: the steps' order, the settling, what it learns).
  - **Not yet run on the Ryzen:** `main` is at ABI 37 and the machine at 35 until its stick is rewritten.
- **What this `usb_host` does differently for devices that are not cameras:**
  - **The bulk stream:** it is made only when a client calls `bulk-stream`. In the tree only `video_gw` calls it: `rg bulk_stream` finds its two calls and the library's wrapper. A bulk stream takes the controller's pages, but leaves 12 of its 64 free, so a device plugged in meanwhile still gets them, and gives them back at its end.
  - **Unchanged code paths:** without a bulk stream, `streams_of`, `end_streams`, `stream_event`, `take_stream`, `queue_td`, `isochronous` and `release` run as before. Each new branch is taken only for a stream marked bulk. An isochronous stream still takes at most 4 pages (`ISO_PAGES`, the old `STREAM_PAGES`).
  - **The one change with no bulk stream:** `bulk` is refused (invalid) on an endpoint that has a stream on the same endpoint number. Before, it put a transfer on the ring that endpoint had before its setting changed.
  - **Why no device of ours meets it:** no device used with MIND Core has a bulk and an isochronous endpoint at one number:
    - the keyboards, mice and tablets have interrupt endpoints;
    - the USB drives have bulk ones;
    - the CH340 and FT232 have bulk ones;
    - the USB audio devices have isochronous ones;
    - Bluetooth's isochronous settings are not selected.
  - **The gate:** the suites `usb`, `ehci`, `usbaudio`, `link`, `iso` (a USB boot disk), `boot` (its EHCI disk) and the USB image's boot ran with this `usb_host`. Their results are in the progress below.
- **What the gate shows:** the `cameras` suite checks two streams of the gateway at once, with the patterns. Nothing in the gate shows two real streams starving each other:
  - QEMU 8.2 has no USB camera (`-device help` lists none);
  - the bulk stream of `usb_host` is not reached by any QEMU device, and is evidence on the Ryzen alone;
  - the suites `usb`, `ehci`, `usbaudio` and `link` passed with the new `usb_host` (LEG9).
- **Tests run on LEG9 on the branch's last code** (15a7d674d):
  - **`cameras`** passed; its `camera -c` part was skipped there.
  - **On a local tree with APP's branch merged** (`claude/wizardly-franklin-kec1a9`, never pushed), `cameras` passed whole: the two patterns through the stub, then through `camera -c`, with 8 s recorded beside a still of the other and the first camera busy for a third.
  - **`bench`, `wm` and `vfs`** passed (33 checks); their camera checks run on the test pattern.
  - **`host_tests.sh`** ran up to its OpenSSH tests, every test before them passing. The OpenSSH tests failed for want of the unpacked `sshd`, which was lost with `/tmp`, and the steps after them did not run.
  - **The host tests this work touches** passed by themselves: `uvc` 19, `video_gw` 9, `video` 6, `idl` 10, `idl_fuzz` 3, `usb_hub`, `iso_ring`, and `tests/idl_test.py`.

**The `cameras` suite** (`tests/qemu_smoke.py`, in the gate's group `x86: keys, shell, tools`):
- **Through a test program of DRV's,** `tests/video_stub` (`videostub CAMERA SECONDS [WxH] [RATE]`, built by `scripts/x86_fixtures.sh`), since `camera -c` (158-APP-0147) is not in `main`: two patterns listed; the first read for 8 s (81 frames) while the second is read for 3 s (28 frames), each its own pattern (the second's counter 300 ahead); the first busy for a third program; camera 2 not found. Passed on LEG9.
- **Through `camera -c`** when the tree has it (the bench diff's checks: a recording of one, a still of the other, the first busy); until then the suite says it skipped that part.
- **What the gate cannot show:** two real streams starving each other. QEMU 8.2 has no USB camera device (`-device help` lists none), and the patterns move no USB transfer. Only the Ryzen's two cameras show it.


## Related

[158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) (every USB camera listed), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md) (`camera -c`, `n`, Tab), [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) (a camera's modes).
