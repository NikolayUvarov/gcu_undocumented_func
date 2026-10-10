# 211-APP-0046 — A window back to its content's size

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) (and [158](158-video-capture.md)) · **Roadmap:** track G · **Constitution:** MC-11.5

Numbered from the kernel track's request in `requests-APP.md` ("A window back to its content's size", 2026-10-10), at the maintainer's request after a run on the MacBook Pro: "the picture should stretch with the window, and the window should be able to take the size of its content, to get the original display back".

## Problem

- `camera` scales its picture to the size `wm` gives its window (the kernel track's change to 158, f760714, on its branch).
- Nothing brings a window back to the size of its content. For `camera` that is the stream's size, 320×240 by default, where the picture is drawn pixel for pixel. `wm` updates `Win::size` when the program takes a new size, so `Win::natural()` follows the current size, not the first.

## Plan

- `wm` keeps the size a pixel window opened at (the content size its program asked for), beside its current one.
- A "fit to content" command sets the frame back to that size, asking the program for it as a resize does, and keeps the frame on the screen: a key (Alt+0) and a double click on the title; the help screen and the top bar's key list name it.
- Text windows: the command takes the frame to the size the window opened at (80 × 25 for most programs).
- **Docs:** `docs/tools` (EN, RU).

## Acceptance criteria

- The `wm` suite resizes `clock`'s pixel window and Alt+0 brings its frame back to its first size (the clock draws at 320 × 176 again).
- With the kernel track's camera change in `main`: the `wm` suite opens `camera` (the synthetic source), resizes its window and sees the picture scaled; Alt+0 brings the frame back to 320×240 and the picture is the test pattern pixel for pixel.
- On the MacBook Pro the FaceTime camera's window returns to its first size (the maintainer's run).

## Related

[158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md), [u002](../issues-done/u002-restore-and-unsnap-windows.done), [u009](../issues-done/u009-pixel-windows-follow-their-frame.done).
