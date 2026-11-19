# KRN landing request: every stop the kernel chooses falls back on a trial and is recorded (801-KRN-0261)

**From:** KRN · **To:** LANDING · **Concerns:** 801-KRN-0261, 801-UPD-0044 (opened), 000-KRN-0217's step 3, 801 S1 and S5

**Ready:** `claude/801-KRN-0261-every-fatal-end` a79037d0b4

## What it carries

**A hole in S1, closed.** On `main`, only a panic on a trial restarted into the fallback (801-KRN-0258). These three called `halt_all`, which stops the watchdog, so a trial that ended in one stayed stopped until a person came:
- an exception in the kernel;
- `init`'s exit;
- an interrupt with no handler.

Now all four take one path:
1. The report's line is kept in a firmware variable.
2. A trial restarts into the fallback.
3. The next boot names the record and deletes it.

**ABI 35:** `BOOT_LAST_RUN_FAULTED` is set in the tasks' `BootInfo` when the last run of this slot ended in a fault of the kernel's own.
- `init` takes it as a freeze's evidence (000-KRN-0217's step 3): what was starting is left out at once.
- `init`'s exit is recorded but is not such evidence.

**The kernel's size:** `kernel/size-budget.json` goes to 8000 in a commit of its own, for 78 lines. Its reason: after such a stop no task runs (MC-1.2).

**The test kernel `panic-test`** also panics on the tick 20 s past the boot that follows its first panic.

The branch also opens 801-UPD-0044. The record's line does not reach the server yet, so S2's report of such a fallback has no step.

## Tested

On `2e2c9772c5`, the last code commit; the commits after it are text only. All of it in QEMU, through Effector:

| Group | Machine | Time | Result |
|---|---|---|---|
| x86: boot, display, shell, memory, clock | i9 | 1692 s | PASS |
| x86: the watchdog | x32 | 289 s | PASS |
| x86: services, storage, audio | x32-2 | 964 s | PASS |
| x86: one CPU | x32-2 | 203 s | PASS |
| host tests | x32 | 1126 s | PASS |
| aarch64: boot and fault containment | PCU | 220 s | PASS |
| aarch64: the updater and the modes | PCU | 1959 s | PASS |

**What the boot group shows:**
- The panic is recorded, and the next boot names it.
- That boot's own panic at 20 s leaves out `video_gw, audio_gw, tts` at once.
- On the panicked trial, slot A names slot B's panic.

The same groups passed on `d3ec178c7b`, before the report code was folded.

It is in `fast-test` (`7b27509aaf`).
