# 093 — `record`: screen and window recording

**Type:** tools · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — for the screen; 088 for a single window · **Roadmap:** track G · **Constitution:** MC-3.3, MC-10.2 (what is on the screen is the user's), MC-2.6

## Problem

`screenshot` (086) takes one frame. There is no way to record what happens on the screen or in one window: a demo, a bug report, the voice dialogue with the shell.

## Plan

- **`record [-w window] [-r fps] [-t seconds] [file]`** in the shell, like `screenshot`. Without `-w` it records the screen in front; with `-w` it records one window of `wm`. It stops on a key (Esc), after the time, or when the disk is full. The default file is the first free `ram:record-NNN.avi`.
- **Frames:**
  - The screen comes through the compositor (`idl/display.wit` 1.1: a `capture` that also says which rectangle changed since the caller's last capture, so unchanged frames cost nothing).
  - A window comes through `wm`, which holds its surface lease from the broker (157) and copies its pixels or renders its cells.
  - Frames are taken at the requested rate (default 10 per second), and an unchanged frame repeats the previous one in the container.
- **Format: AVI with Motion JPEG**, which every player opens. That needs:
  - a small baseline JPEG encoder in libmind (`mind::jpeg`: 8×8 DCT, fixed quality tables, 4:2:0, Huffman with the standard tables);
  - a writer for the AVI index, streaming in 64 KiB writes like `screenshot`.
  
  A frame of the 1280×800 screen is about 50–150 KiB, so ten seconds at 10 frames per second fit the 8 MiB RAM disk. `data/` holds longer recordings.
- **Sound** (optional, later): the audio gateway's playback mixed into the AVI as PCM, so a recording of `say` has its voice.
- **Authority:**
  - `record` needs the shell's compositor client (the screen) or `wm` (its windows). No program records another's window without the user starting it.
  - While recording, a red dot in the corner of the screen tells the user. The compositor draws it, so a recorder cannot hide it.

## Acceptance criteria

- QEMU suite: `record -t 3 data/rec.avi` while `clock` runs. The file read back with mtools is valid AVI/MJPEG: ffprobe on the host, when present, reports the frame count, size and rate. Frames differ as the clock changes.
- A recording of one `wm` window has the window's size.
- Host tests:
  - the JPEG encoder: a decoded test image stays within an error bound of the original;
  - the AVI writer: header, index and chunk sizes.

## Related

[086](../issues-done/086-screenshot.done), [088](088-text-window-manager.md), [157](../issues-done/157-window-broker.done), [158](158-video-capture.md).
