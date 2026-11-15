# 000-KRN-0254 — In the USB image check, a `run memmap` refused later than the harness's 8 s on a loaded lane

**Type:** shell, console (`tests/usb_image_smoke.py`'s unplug check, 000-KRN-0085) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (2026-11-15; cause found, the check's fix made, its ten runs left) · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

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

**Corrected on 2026-11-15** (the record above misread the log). Read again, with the stuck record (`mind-core-stuck-2284066-1.log`):
- **No line came out of order.** `ps`, `logs 1`, `ps` and `logs 7` were the check's own commands before the `run memmap`: `service_logs("init")` and `service_logs("usb_host", "DISCONNECTED")` after the fourth unplug (`device_del usbstick5`). They are not the harness's notes, which read QEMU's monitor and type nothing.
- **The refusal came late.** The command was `unplugged()`'s first `run memmap` after that unplug (line 355). The harness waited its default 8 s for the prompt.
  - The record at the 8 s mark, guest time 76 s: the shell waits for the loader's reply, the loader for `vfs_server`'s, `vfs_server` for `usb_storage`'s, and `usb_storage` sleeps between its claims (up to 20, 100 ms apart, on each request to a gone device).
  - One second later, at 78 s, every task waits to receive: the chain has answered. The log then shows the refusal and the prompt, before QEMU was stopped.
- **That boot was slow.** Its first refusal took 5.9 s. In the 40 runs kept on x32 the first took 2.0–5.9 s: 4.0 s in 28, 5.9 s in two, this one among them.
- **The check was stricter than its criterion.** [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md) asks for the refusal within 15 s; the check took the harness's 8 s.

## Plan

1. ~~Find where a typed line waits while background programs hold `CONSOLE=1`.~~ No line waited: the reorder is ruled out (above).
2. The check waits for each refusal up to 211-KRN-0050's 15 s, and prints the slowest, so a refusal that slows down shows in the log before it fails.
3. Ten runs of `x86: USB image` on x32 beside a busy lane.

## Acceptance criteria

- The cause named, with a run that shows it, or the reorder ruled out with the reason.
- `x86: USB image` passes ten times on x32 beside a busy lane.

## Progress

- **2026-11-15, the fix** (`claude/000-KRN-0254-a-refusal-within-its-limit`). `tests/usb_image_smoke.py`:
  - each `run memmap` refused in `unplugged()` waits up to `REFUSED_WAIT`, 15 s, 211-KRN-0050's limit;
  - each refusal's time is kept, and the check prints them all, the slowest beside the limit.
- **2026-11-15, i9** (`347b34a948`, alone): `x86: USB image` passed in 225 s. The first refusal took 5.9 s, the slowest of the four 9.8 s: the harness's 8 s would have failed it here too.
- **Not known yet:** why a later refusal takes longer than the first.
- **Round 143's gate, a second cause in the same group** (x32-2, candidate `a29ba720b3`): `debug_mode()` failed in 117 s.
  - It looks for the `uptime` record in `cat log:<boot log>`. `cat` shows a file's first 16 KiB on the screen, and that slow boot's log was 194 bytes longer: the record was past the end shown.
  - The fix: it reads the log whole through a pipe (`cat log:<name> | grep uptime`, `| grep DEBUG`), as `unplugged()` already did.
- **Round 143's gate, a third cause:** the repeat alone on x32 failed in `unplugged()`, at the copy one MiB longer.
  - `device_del` of the stick and `device_add` of the copy went at once. A request reached `usb_storage` before `usb_host` had the copy set up: `usb_storage` was asleep in its claims before the check's first `run memmap`.
  - In its 2 s of claims it found neither, said `THE DEVICE IS GONE`, and from then on it names no other medium: its `lost` mark covers both lines. So `ANOTHER MEDIUM (... SECTORS) IS NOT USED` never came, in 15 tries.
  - The fix: the copy goes in first, and the stick comes out once `usb_host` has the copy set up. The request that finds the stick gone finds the copy at once. They are two files, so QEMU's image locks allow both at once.
  - **Left as it is:** the same stick out and in again "at once" a little earlier has the same race. It cannot overlap, since one image file cannot be attached twice. It has passed in every run so far.
  - **For DRV** (`usb_storage`): a medium of another capacity found after a loss could still be named once. It would tell the bench what a reader found.
- **Left:** the ten runs on x32 beside a busy lane, each refusal's time recorded.

## Related

- [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md), the unplug check's task.
