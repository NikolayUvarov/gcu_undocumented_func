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

## Progress (2026-10-13, plan step 1: the compositor's half)

- **`compositor` logs its own half** without a new interface:
  - once at start: `[COMPOSITOR] SCREEN 2560X1440, STRIDE 2560, FORMAT BGR`;
  - each time another screen comes in front: `SCREEN IN FRONT #N: FIRST FRAME IN … MS, … OF … PIXELS NOT BLACK`. At boot, #1 is the shell's;
  - for 10 s after that, each second in which the screen changed: the frames, the longest copy, the pixels written and those not black;
  - at most 24 lines in all.
- **Reading the next run's log:**
  - no `SCREEN IN FRONT #1` line: the shell's screen never came in front;
  - #1 with 0 pixels not black, and no frames after it: the shell did not draw (the shell's own first-frame line, asked of the tools track, will say so);
  - frames and lit pixels but a black screen: the path to the framebuffer, or the display itself.
- **Suspect 3 is weaker than it looked:** since 000-KRN-0081 the kernel maps the compositor's framebuffer write-combining through the PAT, which takes the place of the MTRRs' uncachable default.
- **The keys on boots 1–3 may be a reaction, not a cause:** a black screen invites a key press, and no key came on boot 5, whose screen worked. The next run notes when the screen went black and when a key was first pressed.
- **In QEMU** (`display` suite, VGA std, 1280 × 800) the shell's screen came in front at 0.4 s with 0 pixels not black, and its first drawn frames followed within the second, all 1024000 pixels not black: the shell's background is not pure black. The suite checks the two lines on each of its three adapters.

## Progress (2026-10-13, evening: the cause found and fixed)

**The maintainer's runs of `fast-test` 29fee23,** which has the compositor's lines:
- **The MacBook Pro** (boot 1) drew as before. Its shell's first frame had 208424 pixels not black, then thousands of pixels were written each second.
- **The Ryzen machine** (boots 2 and 3, the stick in a blue port and then a black one) showed a black screen both times. Its screen is now 3440 × 1440, stride 3456: another monitor. The compositor logged `SCREEN IN FRONT #1: FIRST FRAME IN 7 MS, 0 OF 4953600 PIXELS NOT BLACK` when the shell started, and then no second in which the screen changed.
- **So the shell never drew.** No USB keyboard was found on either boot, so no key was pressed: the keys were not the cause.

**The cause: COM1.**
- The shell reads its serial line in `while let Some(byte) = uart.read()` before it renders.
- A 16550's line status says whether a byte is ready. Ports that nothing decodes read 0xFF, which says "ready" for ever, so the loop never ended and the first frame never came.
- The Ryzen machine's ACPI tables list no COM port (no PNP0501). The Intel Core i5's do (its board has a 16550). The MacBook Pro's list none either, but its ports evidently do not read all ones, as its shell draws.
- **Reproduced in QEMU** with no 16550 (`-serial none`): the screen stayed black, 0 of 1024000 pixels lit after 45 s. With one, every pixel was lit.

**The fix:**
- `mind::dev::Uart::open` returns None where no 16550 answers: its scratch register does not keep what is written, or its line status reads 0xFF. `read` takes a status of 0xFF as no byte.
- `init` lends the shell no serial line in that case and logs `[INIT] NO SERIAL LINE: NO 16550 ANSWERS AT COM1 (3F8), THE SHELL GETS NONE`.
- **The `display` suite** boots once with no COM1 and checks through QMP's screendump that the shell draws.

**Not explained yet:** boot 5 drew the shell, and then its keyboard typed nothing. A line that read "no byte" at first and "ready" later would do both. The maintainer's next run will tell.

**Left for the acceptance criteria:** the shell's text on this machine's screen.

**2026-10-14:** the compositor's lines and the fix for a PC without COM1 are in `main` (8ae66ed), gated on i9 (x86) and PCU (aarch64), and in `fast-test`. What is left is the maintainer's run on the Ryzen machine.
