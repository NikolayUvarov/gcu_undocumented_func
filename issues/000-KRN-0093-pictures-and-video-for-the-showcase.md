# 000-KRN-0093 — Pictures, a row of frames and a video of MIND for the showcase page

**Type:** host tools (`scripts/`), the showcase's documentation · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — (the tools track's request below makes shooting easier, not possible) · **Main task:** — · **Constitution:** MC-12.3

## Problem

The maintainer (2026-10-12) wants for the project's page on GitHub ([000-KRN-0088](000-KRN-0088-mind-core-os-a-copy-under-the-os-name.md)):

1. photos of the interface;
2. a screencast as a row of pictures;
3. a video.

**What the system has today:**

- `screenshot [file]` writes the screen in front as a 24-bit BMP. Without a file it goes to `ram:screen-NNN.bmp`, which is lost at a reboot. `wm`'s menu has it under System.
- `record [-w] [-r frames per second] [-t seconds] [file]` writes an AVI of Motion JPEG frames, quality 75. While it records, the compositor shows a red dot in the screen's corner (issue 165). With `-w` from `wm`'s run line it records the window in front alone.

**What is missing:**

- GitHub shows PNG, GIF, WebP and MP4, not BMP or Motion JPEG AVI.
- The MacBook Pro's screen is 2880 × 1800: a BMP is 15 MB, and a recording is large.
- A screenshot taken from the shell shows the shell.
- The pictures cannot be made again the same way for each update of the page.

## Plan

1. **`scripts/media.py`** (host, Python; `ffmpeg` for video):
   - `shots`: each BMP to PNG, and a copy at most 1600 px wide;
   - `series`: the frames of an AVI, or a list of BMPs, to PNG files, and to an animated WebP and GIF;
   - `video`: an AVI to MP4 (H.264, yuv420p, `+faststart`), sized to GitHub's limit for an attached video;
   - each output's size printed.
   - The red dot stays in the frames: it is the recording indicator, not a defect.
2. **`scripts/qemu_scenes.py`:** the current build in QEMU (x86, 1920 × 1080, the test suites' VM class and its QMP screen dumps) plays fixed scenes and saves them as PNG:
   - the boot screen;
   - `wm` with windows (`clock`, `fm`, a console);
   - the shell with `check`'s table;
   - `top`;
   - one scene as a row of frames.

   The same pictures each time, for each update of the page.
3. **Documentation:** how to shoot on a real machine (the commands, `data/` so the files survive a reboot), how to convert, and where the results go:
   - pictures in the showcase's `docs/media/`, each under 1 MB;
   - videos attached on GitHub, not committed.
4. **The tools track's part** goes to [requests-APP.md](requests-APP.md): a screenshot by a key, without the shell on the screen; a delay; files kept in `data/media/`; a smaller recording on a Retina screen.

## Acceptance criteria

- `media.py` turns a BMP and an AVI made by the MacBook Pro into PNG, WebP or GIF, and MP4, each within GitHub's limits. A host test covers the BMP and AVI readers on small files the test makes.
- `qemu_scenes.py` makes every scene on i9 in under 10 minutes.
- The page shows at least one picture from the real machine and the QEMU scenes. Each says where it was taken (MC-12.1).

## Related

[000-KRN-0088](000-KRN-0088-mind-core-os-a-copy-under-the-os-name.md), [docs/tools/README.md](../docs/tools/README.md) (`screenshot`, `record`), [requests-APP.md](requests-APP.md).

## Progress (2026-10-12)

- **`scripts/media.py`** (`shots`, `series`, `video`): its own BMP and AVI readers and PNG writer in Python; ffmpeg scales and writes WebP, GIF and MP4. On a 2880 × 1800 BMP made for the test, the PNG is the BMP pixel for pixel (both decoded by ffmpeg, the same SHA-256), 0.58 MB, and the 1600-wide copy 0.29 MB. A 3-second 2880 × 1800 AVI at 10 frames a second became an MP4 of 1.02 MB, 1920 × 1200, yuv420p.
- **`tests/media_test.py`** (in `scripts/host_tests.sh`): the readers and the PNG writer on small files it makes, the AVI laid out as `libmind/src/avi.rs` writes it, with a repeated frame, an odd-length frame and a recording cut before its final header; the three commands where ffmpeg is installed. 8 tests pass here.
- **`scripts/qemu_scenes.py`:** `boot`, `check`, `top`, `wm` and `series` at 1920 × 1080 (QEMU's VGA offers the size through EDID and OVMF takes it). Here (QEMU 8.2, TCG, 4 CPUs) every scene took 29 s.
- **The guide:** [docs/media.md](../docs/media.md) and [docs/media_RU.md](../docs/media_RU.md), linked from the README.

- **The page's animation** (the maintainer, 2026-10-12: "make a nice animation of them and put it on GitHub"): `qemu_scenes.py --scenes tour` plays the boot, `check kernel`, `hw`, `top`, `wm` and three of its windows full screen, and `media.py slideshow` joins them, each going into the next through dark, looping. `docs/media/tour.webp`: lossless, 1280 × 720, 8 pictures, 24.6 s, 1.39 MB (a crossfade of two screens of text came to 2.3–4.7 MB). The README shows it under its intro, saying it is QEMU and which build. The tour took 66 s here.

**Still open:** the scenes on i9 (the second criterion); a picture from a real machine on the page (the third).

## Progress (2026-10-14)

- **The page's animation** (`docs/media/tour.webp`, the QEMU tour made by `qemu_scenes.py --scenes tour`, built from 73b3d33) is in `main` (8ae66ed), under the README's title, with its caption naming QEMU and the build.
- **Left for the criteria:** a picture from a real machine on the page (`screenshot` or `record` on the MacBook Pro, then `media.py shots`).
