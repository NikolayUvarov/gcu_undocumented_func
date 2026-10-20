# 000-APP-0089 — Screenshots and recordings for the project's page

**Type:** tools (`shell`, `screenshot`, `record`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** PrintScreen: a key code from `KRN` ([requests-KRN.md](requests-KRN.md), "A key code for PrintScreen") · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.4

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

## Progress (2026-10-20)

- **The key is Ctrl+Alt+F6, not Ctrl+Alt+S.** `INPUT_LISTEN` matches a key code and modifiers, and every character is `KEY_CHAR`: listening for Ctrl+Alt+S would take every Ctrl+Alt+letter from the program in front, with no way to give the others back. F6 is a key code of its own, beside the consoles' Ctrl+Alt+F1…F5, and the MacBook Pro's keyboard has it. PrintScreen has no key code (`libmind/src/keys.rs` drops `E0 37`): asked of the kernel track in requests-KRN.md.
- **`shell/src/screenshot.rs`, `shell/src/main.rs`:**
  - Ctrl+Alt+F6 takes the screen in front at once; the line `[SHELL] SCREENSHOT <file>: WxH, N BYTES (CTRL+ALT+F6)` goes to the system log, and to the shell's screen when the shell is in front. A program in front keeps its screen as it is: no note is drawn over it.
  - `screenshot -d <1-600>` takes it that many seconds later; the shell goes on meanwhile, so another program can be brought to the front (`fg`).
  - Without a file: the first free `data/media/screen-NNN.bmp` (the directory made if need be), or `ram:` when `data/` cannot be written.
- **`record`:** `-s <1-4>` writes frames that many times smaller in each direction (`record/src/shrink.rs`: each block averaged per colour), `-q <1-100>` the JPEG quality (75); without a file `data/media/record-NNN.avi`.
- **Host tests:** `tests/record_host.rs` (a block averaged, the pixels that make no whole block left out, rows packed).
- **The `vfs` suite** (raw FAT disk): `screenshot` writes `data/media/screen-001.bmp`; `screenshot -d 1` writes `screen-002.bmp` later; Ctrl+Alt+F6 with clock in front writes `screen-003.bmp`, which matches QEMU's picture of clock's screen (at least 90% of the sampled pixels; no shell in it); `record -s 2` writes an AVI of half the screen's width and height; `record` alone writes `data/media/record-001.avi`.
- **Docs:** the help line, `docs/tools` (EN, RU).
- **Left:** PrintScreen (the kernel track's key code); the gate on this batch; the MacBook Pro's picture and 30 s recording.

## Related

[000-KRN-0093](000-KRN-0093-pictures-and-video-for-the-showcase.md), 086 (`screenshot`), 093 (`record`).
