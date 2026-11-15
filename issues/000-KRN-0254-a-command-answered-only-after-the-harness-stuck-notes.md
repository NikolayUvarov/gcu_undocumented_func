# 000-KRN-0254 — In the USB image check, a `run memmap` echoed and answered only after the harness's notes of a stuck wait

**Type:** shell, console (`tests/usb_image_smoke.py`'s unplug check, 000-KRN-0085) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (2026-11-15) · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**Round 141's gate** (candidate `87253d9a12`), `x86: USB image` on x32-2, failed in 83 s in `unplugged()`: `Timeout waiting for 'MIND> ': run memmap`. The repeat alone on x32 passed in 170 s. The logs are on x32, `~/mind-core/KRN/logs/land-r141/x32-2-14/`.

**What the QEMU log shows** (`mind-core-usb-image.log`), at its end:
- the fourth stick unplugged (`[USB] PORT 4: DISCONNECTED`);
- then the harness's notes of the stuck wait: `ps`, `logs 1`, `ps`, `logs 7`;
- **only then** the echo of `run memmap`, its answer `ERROR: CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)` and the prompt.

So the line the check typed was echoed and run only after the lines typed later. At the time of the notes:
- every task waited (`IPC_WAIT`), the shell was running `ps`, and the CPUs were halted;
- two `memmap` from the check's earlier `run memmap &` were `SLEEPING` with `CONSOLE=1`.

**Not known yet:**
- **Where the line waited:** in the console's input, with a background program holding the console, or in the shell.
- **Why it came out after the later lines.**

A line typed at the console must reach the shell, or the program in front, in the order it was typed.

## Plan

1. Find where a typed line waits while background programs hold `CONSOLE=1`.
   - Read the console's input path: `usb_serial` or the serial line, the console writer, the shell.
   - Try to reproduce it: `run memmap &` twice, a stick unplugged, then `run memmap` repeated under load beside a busy lane.
2. Fix the order in the component that breaks it.
3. If the check itself races, fix the check. It must not hide a real reorder.

## Acceptance criteria

- The cause named, with a run that shows it, or the reorder ruled out with the reason.
- `x86: USB image` passes ten times on x32 beside a busy lane.

## Related

- [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md), the unplug check's task.
