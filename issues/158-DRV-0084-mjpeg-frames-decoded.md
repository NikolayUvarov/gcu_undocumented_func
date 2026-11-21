# 158-DRV-0084 — MJPEG frames decoded, so a camera's larger modes are offered

**Type:** driver (`video_gw`, a decoder in `libmind`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** in progress (2026-11-21: the decoder hardened, given the standard tables and a faster IDCT, and fuzzed; `video_gw`'s side next) · **Blocked by:** [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-10.1, MC-12.1

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
- [ ] On the Ryzen an MJPEG mode of each camera is opened and shown, with its frame rate and decode time recorded (MC-12.1).

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
- **Not done yet:**
  - frames recorded from the Ryzen's cameras: the machine runs ABI 35 and `main` 37 until its stick is rewritten;
  - the decoder in ASR's list in `docs/assurance/README.md`, which is ASR's to write (a request to its owner);
  - `video_gw`'s side.

## Related

[158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md), [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md).
