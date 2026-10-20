# 000-APP-0089 — Screenshots and recordings for the project's page

**Type:** tools (`shell`, `screenshot`, `record`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.4

**Recorded by:** the kernel track (KRN), 2026-10-12, in `requests-APP.md`, for [000-KRN-0093](000-KRN-0093-pictures-and-video-for-the-showcase.md), at the maintainer's request. Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

The maintainer will shoot the interface on the MacBook Pro for the page on GitHub: pictures, a row of frames and a video. `screenshot` and `record` capture, but:

- **the shell is in the picture:** a screenshot is typed in the shell, and `wm`'s menu entry opens over the screen;
- **the files are lost at a reboot:** without a file name both write to `ram:`;
- **a Retina recording is heavy:** at 2880 × 1800 each Motion JPEG frame is large, and the encoder may not keep 10 frames a second.

## Plan

- **A key for a screenshot** that saves at once, without the shell or a menu on the screen:
  - PrintScreen on a PC keyboard, and Ctrl+Alt+S for the MacBook Pro, which has none;
  - the shell listens for them whatever program has the keyboard, as for Ctrl+Alt+F1…F5;
  - a short note in the corner says where the file went.
- **`screenshot -d <seconds>`:** waits first, so the screen can be arranged.
- **Files kept by default** in `data/media/` (`screen-NNN.bmp`, `record-NNN.avi`), so they survive a reboot and can be read from the stick in Windows; `ram:` when the boot disk is read-only.
- **`record -s 2`:** half the size in each direction. **`record -q <quality>`:** the JPEG quality.
- The help and `docs/tools` (EN, RU).

## Acceptance criteria

- **The `tools` suite:**
  - the key writes a BMP to `data/media/` with the screen as it was, with no shell in it;
  - `screenshot -d 1` writes after a second;
  - `record -s 2 -t 3` writes an AVI whose frames are half the screen's size.
- **On the MacBook Pro**, the maintainer takes a picture and a 30 s recording without the shell in them.

## Related

[000-KRN-0093](000-KRN-0093-pictures-and-video-for-the-showcase.md), 086 (`screenshot`), 093 (`record`).
