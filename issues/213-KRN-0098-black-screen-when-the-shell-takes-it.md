# 213-KRN-0098 — The screen goes black when the shell takes it (Ryzen 9 5900HX, Radeon iGPU, 2560 × 1440)

**Type:** `compositor`, the kernel's screen, diagnostics · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — (the next run's evidence) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1

## Problem

On the maintainer's Ryzen 9 5900HX machine (2026-10-13, `fast-test` 28d9c3d) the kernel's and the services' white lines show on the screen, and when the shell takes the screen it goes black: the shell's green text never appears. The system log shows nothing wrong: every service started, the shell at 7.8 s, and init went on (the legacy report, the recovery reserve, the hardware report).

**The screen as the kernel saw it:** 2560 × 1440, stride 2560, format 1 (BGR), at 0xD0000000, inside the GPU's 256 MiB prefetchable BAR0. The MTRRs cover 0–3.25 GiB as write-back and leave the rest uncachable by default (no write-combining range); the PAT has write-combining in PA1.

**What is known from the evidence:**
- **The framebuffer takes writes:** the kernel's lines and the black after them both reached it.
- **The shell ran:** it started, but nothing says whether it drew its banner or kept running.
- **Neither `compositor` nor the shell logs anything on this path**, so the log cannot tell them apart.

**Suspects, in order:**
1. **The compositor shows something other than the shell's screen** (the focus, the frame it copies, or the conversion at this size or format).
2. **The shell stopped before its first render** (a call it waits on at start).
3. **The copy to an uncachable framebuffer** is far slower than on the MacBook Pro (14.7 MB a frame), so a frame never seems to finish.

## Plan

1. **Diagnostics in the build:**
   - `compositor` logs the screen it found (size, stride, format, address and the mapping's memory type), the task whose screen it shows when the focus changes, and how long a whole frame takes to reach the framebuffer, once a second at most;
   - the shell's first rendered frame is asked of the tools track as one log line.
2. **The maintainer's next run** (no screen needed):
   - after the screen goes black, type `screenshot data/black.bmp`, Enter, wait 10 s;
   - then type `reboot`, Enter;
   - send `data/black.bmp` and `log:`.
   - A machine that restarts shows that the shell is alive. The BMP shows what the compositor holds: green text points to the path to the framebuffer, black to the shell or the focus.
3. **The fix** follows the evidence. If the framebuffer is slow, the compositor maps it write-combining and copies only the rows that changed.

## Acceptance criteria

- On this machine the shell's text shows after boot.
- The system log names the screen, the task shown and the frame time.
- The QEMU display suite passes on its three adapters.

## Related

[213](213-amd-ryzen-pc.md), [000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done), 211-KRN-0017 (service logs on the boot screen), 211-KRN-0018 (the compositor's quota and the screen).

## Progress (2026-10-13, a fifth boot)

- **The same machine showed the shell's green screen** on its fifth boot (`log:boot0005.log`, `hw0005.txt`: the same Ryzen 9 5900HX, devices and 2560 × 1440 screen, the same build 28d9c3d). The black screen is therefore not certain on this machine: it came on boots 1–3 and not on boot 5.
- **What differed in the logs:** on boots 1–3 a key report came from the USB keyboard (at 25.6, 17.2 and 8.8 s; space and F2 among them). On boot 5 none came before the log ends at 13.8 s. A key pressed while the screen changes hands stays a suspect next to the three above; whether the keys came before or after the black is not in the log.
- **The plan stands.** The next run also notes when a key was first pressed.
