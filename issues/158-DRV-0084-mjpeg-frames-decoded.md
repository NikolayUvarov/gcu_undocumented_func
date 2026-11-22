# 158-DRV-0084 — MJPEG frames decoded, so a camera's larger modes are offered

**Type:** driver (`video_gw`, a decoder in `libmind`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** in progress (2026-11-22: the decoder hardened, given the standard tables and a faster IDCT, and fuzzed; `video_gw` streams and decodes MJPEG modes, `video.wit` 1.2; MJPEG shown on both cameras in the maintainer's look; frames recorded and compared with Linux's decoding, and ASR's list, to come) · **Blocked by:** [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-10.1, MC-12.1

## Problem

On USB 2 a camera sends its larger sizes and higher rates as MJPEG alone: uncompressed, they do not fit the bus. `video_gw` decodes none: a camera without YUY2 gets the line `HAS NO YUY2 FORMAT (MJPEG IS NOT DECODED YET)`, and with [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) its MJPEG modes are listed as not offered.

## Plan

1. **Choose the decoder and say why:** a baseline JPEG decoder of our own (sequential, 8 bit, 4:2:2 and 4:2:0, the standard's Huffman tables when a frame carries none, as UVC allows), or a third-party crate with its source and licence in `THIRD_PARTY.md`.
2. **The input is the device's:** every length and table is checked, and a malformed frame is dropped with a count, never trusted. The decoder goes into the fuzzing list (`ASR`).
3. **`video_gw`** decodes into the size asked and offers the MJPEG modes.
4. **Measure** the time a frame takes to decode on the Ryzen at each size: a mode the machine cannot decode at its rate is offered at the rate it reaches, and the list says so.

## Acceptance criteria

- [ ] Host cases: frames recorded from the Ryzen's two cameras decode to the picture Linux decodes, within a stated error; truncated and corrupted frames are dropped.
- [ ] The decoder is fuzzed with the other parsers.
- [x] On the Ryzen an MJPEG mode of each camera is opened and shown, with its frame rate and decode time recorded (MC-12.1). (2026-11-22: shown in the maintainer's look, both cameras at 1280×720; the rates and decode times are in progress item 4: camera 0 alone 12 ms a frame at 1280×720 and 15 frames a second in that light, camera 1 64 ms alone and 24 to 30 ms beside camera 0 at 12 to 15 a second.)

## Progress (2026-11-21)

**1. The decoder: `libmind/src/jpegdec.rs`, the baseline decoder `wm`'s backgrounds already use (000-APP-0050), taken on.**
- **Why not a third-party one:** it already decodes 8-bit sequential pictures of any sampling factors with restart markers, row by row, with no system calls and no dependency. A crate would add a dependency and a licence for what is left: the tables, the bounds and the speed.
- **What changed:**
  - **The standard Huffman tables** (JPEG Annex K.3, from `jpeg.rs`) are taken for indexes 0 and 1 when a picture carries none, as a UVC camera's frames may.
  - **A picture whose coded data runs out is refused** (`CUT_SHORT`), where the decoder used to read zeros and show the rest grey. Zero bytes stand for what follows the data or a marker, and taking one of them refuses the picture: at a restart and at the end of each row of blocks.
  - **Bounds, as baseline 8-bit allows:**
    - a DC difference of at most 11 bits and an AC value of at most 10;
    - dequantized values kept to 16 bits, as libjpeg keeps them;
    - one frame header.
  - **The inverse DCT** is the one libjpeg's `jidctint` computes (Loeffler, Ligtenberg and Moschytz's 8-point transform, the inverse of `jpeg.rs`'s forward one), in 64-bit integers, with a block of DC alone filled at once. Before, it summed 64 products per sample.
  - **The colour conversion** looks each component's sample up in a column table made once, instead of two divisions a pixel.
- **Host cases** (`tests/image_host.rs`):
  - **No tables:** a frame of `jpeg.rs` with its DHT segments removed decodes to the same pixels as with them.
  - **4:2:2 without tables, as cameras send:** a hand-built frame decodes to its blocks within 1 of JFIF's YCbCr in floating point.
  - **Against libjpeg (Pillow's `.rgb` files):** the full-resolution pictures `444.jpg`, `big444.jpg` and `halves.jpg` are within 1, and `grey.jpg` matches exactly. The old IDCT was 2 off.
  - **Cut short:** every cut of three frames inside their coded data is refused; a frame that lacks only its end marker decodes.
  - **Hostile values:** a DC table with a 15-bit difference is refused, and quantizers of 65535 decode without overflow.
  - **Seeded fuzzing** (`mutated_frames_never_panic`, `MIND_FUZZ_SEED` and `MIND_FUZZ_ITERATIONS`, 3000 inputs in the host tests): bits flipped, bytes changed, inserted, removed, cut, in seven pictures with and without tables. No panic, rows in order, a decoded picture within its limit. It also ran 200 000 inputs at its seed and 100 000 at five more seeds, with overflow checks on (LEG9).
  - **The old decoder against these cases:** it panicked under fuzzing (`attempt to subtract with overflow`: a DC size beyond 32 bits taken from 32 held). In a release build that would wrap rather than panic.
- **Speed, natively on LEG9** (Intel Core Ultra 9 275HX, Linux, `-O`, `jpeg.rs`'s 4:2:0 of a noisy picture at quality 80, 20 decodes each):

  | size | before | after |
  |---|---|---|
  | 640×480 | 5.7 ms | 3.9 ms |
  | 1280×720 | 16.7 ms | 11.4 ms |
  | 1280×1024 | 23.2 ms | 12.5 ms |

  This is not the Ryzen's figure, nor MIND Core's: criterion 3 asks for that one.

**3. `video_gw` with Motion JPEG.**
- **The frame size streamed** (`uvc::Camera::choose`, now given the rate as well): of the YUY2 and MJPEG sizes, the smallest that holds the picture and reaches the rate asked. YUY2 comes first at one size, since it needs no decoding. Else the one of the highest rate that holds it, else the largest.
  - The Ryzen's camera 0 (host case, from its descriptors): 1280×720 at 25 a second as MJPEG, at 10 as YUY2; 800×600 at 25 as MJPEG, where YUY2 gives 20.
- **An MJPEG frame is assembled** at any length up to the committed `dwMaxVideoFrameSize`, but at most three bytes a pixel and at least 1 KiB (`FrameSize::buffer`): a camera's claim is not trusted further.
- **On a read**, its picture is decoded and each row scaled into the size asked (`video::scale_row`, YUY2's nearest-pixel mapping). Every camera's stream is pumped every 64 rows (`convert`), as YUY2's bands are.
- **A frame that does not decode is dropped:** its header names another size, it is cut short, or it is malformed. It is counted, the first reason is logged (`A MOTION JPEG FRAME DROPPED: …`), and the read waits for the next frame. The picture is not shown again meanwhile, since it may hold some of the dropped frame's rows. Five seconds with no frame shown answer not-found, as five seconds with no good frame do.
- **A stream's close line** adds `MJPEG: n DECODED IN a MS EACH, THE LONGEST b MS, d DROPPED (THE FIRST: …)`: the decode time criterion 3 asks for.
- **`modes` offers the MJPEG modes** within the sizes and rates `open` takes (1280×1024 at most). A compressed mode's bytes are not checked against an isochronous setting: the probe's answer chooses the setting.
  - The Ryzen's camera 0 now offers 20 modes (10 MJPEG), and camera 1 offers 22 (11 MJPEG), by host case.
  - The 1920×1080, 1600×1200 and 2048×1536 ones are past what `open` takes.
- **`video.wit` 1.2** says so: MJPEG modes offered and decoded, nothing new on the wire, so a 1.1 program works with a 1.2 gateway.
- **Host cases:**
  - the choice (`tests/uvc_host.rs`), the buffer, and the offered lists of the two cameras (`tests/video_gw_host.rs`);
  - `scale_row` against YUY2's sampling, up and down;
  - the test pattern encoded by `jpeg.rs` and decoded into the picture as the gateway does (`tests/video_host.rs`): at its own size its frame counter reads back and the mean error is under 6; scaled, each pixel is near the one it samples.
- **Not in the gate:**
  - no QEMU device sends MJPEG, so the gateway's MJPEG path is evidence on the Ryzen alone, and not yet that;
  - the `cameras` suite runs the patterns.

**4. On the Ryzen, 2026-11-21 18:43-19:01 UTC** (release 202611211819, ABI 37; `video_gw` by `replace`, `tests/video_stub` from the console):
- **The lists:**
  - camera 0 `MODES: 26, 15 OFFERED (5 MJPEG)` and camera 1 `24, 17 OFFERED (6 MJPEG)`, as the host cases say. The MJPEG ones are 1280×1024, 1280×960, 1280×720, 1024×768 and 800×600 (and 960×720 on camera 1), at 25 and 30 a second, where YUY2 gives 5 to 20.
  - The first run, before `opens_as`, offered every MJPEG mode up to 1280×1024 and opened the ones YUY2 also gives as YUY2 (`640x480 AT 25/S; MODE 13`). That is why the list now leaves them out.
- **Decode times, camera 0 alone, each MJPEG mode for 2 s** (run 3):

  | mode | decode |
  |---|---|
  | 1280×1024 | 16 ms (at most 18) |
  | 1280×960 | 15 ms |
  | 1280×720 | 12 ms |
  | 1024×768 | 10 ms |
  | 800×600 | 6 ms |

  - 24 frames shown in 1.6 s in each, the camera's 15 a second in that light, with reads at most 62 to 64 ms apart.
- **The same modes in other runs took 24 to 122 ms a frame on average, and up to 325 ms:** camera 0's 1280×1024 alone in 35 ms, camera 1's 1280×720 alone in 64 ms, and 110 to 122 ms beside camera 1 in the first run. The Wi-Fi driver logged late frames in those seconds (8 in one 15 s run, 2 in run 3). That may explain the spread, but it is not shown.
- **Both cameras in MJPEG 1280×720 at once, 15 s:**
  - before b232de0db the gateway ended: `PANIC: memory allocation of 269328 bytes failed`, since each stream reserved the camera's 1.8 MB maximum twice;
  - with it: camera 0 decoded 193 frames in 24 ms each, camera 1 176 in 30 ms each, 1 dropped each (`A FRAME OF ANOTHER SIZE, OR NO FRAME HEADER`), and reads were at most about 0.5 s apart.
- **A mode the machine cannot decode at its rate:** none found at 15 frames a second. The cameras' 25 and 30 were not reached in this room's light, whatever the decode time.

**Not done yet:**
- frames recorded from the Ryzen's cameras and compared with Linux's decoding (criterion 1): the machine runs ABI 35 and `main` 37 until its stick is rewritten;
- an MJPEG mode of each camera opened there, with its rate and decode time (criterion 3), and a mode the machine cannot decode at its rate offered at the rate it reaches (plan item 4);
- the decoder in the fuzzing list of `docs/assurance/README.md` (criterion 2): that file is ASR's, and the request to its owner is in the report.

**The maintainer's look, 2026-11-22** (the Ryzen at its keyboard, release 202611212244 with 5145dab32's `usb_host` and `video_gw` as the boot image's services, the two cameras `0471:A6CA` and `0471:A6CD`; the steps the owner session passed to him from CAM's check). His words: «1, 2. всё так; 3. всё так; 4. всё так; 5. выход из камеры работает. Проверку считаю успешной.» That is, by his own look:
1. **Two windows:** `wm camera -c 0, camera -c 1` showed both cameras at once, both moving.
2. **The next-camera key:** `n` in one window, while the other window kept moving with no pause. Camera 1 being held by the other window, the program closed its camera, was refused the other and opened its own again (the deferred re-open); the freeze he had seen on 2026-11-21 is gone.
3. **MJPEG:** camera 0 at `-z 1280x720 -r 25`, which the gateway streams as MJPEG (YUY2 gives 10 a second at that size).
4. **Both cameras in MJPEG at once:** both at 1280×720.
5. **Leaving:** Esc or `q` ends a camera.

This holds for that machine, those cameras and that release (MC-12.1).

## Related

[158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md), [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md).
