# 000-APP-0131 — The `listen` suite's checks after the recording hold beside a build load

**Type:** tools (tests: the `listen` suite) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [000-APP-0114](000-APP-0114-hear-waits-for-a-whole-command.md) · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.2

**Numbered by:** the tools track, 2026-11-10, from DRV's note on [000-DRV-0066](../issues-done/000-DRV-0066-the-ac97-capture-ring-overflows.done) (`internal-dev-messages`, `APP/2026-11-09T2217Z-DRV-…`).

## Problem

- **DRV's fix** keeps AC'97's capture in `audio_gw`'s own copy, so the suite's recording no longer overflows: none in 20 runs beside a build loop (load 65–75 on 24 CPUs).
- **The suite still failed** in 11 of those 20, after the recording, in this track's checks:
  - 8 timed out waiting for the shell's prompt after `hear --wav nosuch.wav`, `voice on --wav`, `hear x y` and `run keys`. `vm.command`'s wait is 8 s;
  - 3 failed `HEAR: THE MICROPHONE IS BUSY`. The check starts `listen 10` in the background and runs `hear 1` at once. If `hear` asks before `listen` holds the microphone, or after its 10 s, the microphone is free.
- **What these checks test** is an answer's text, not its time: a longer wait does not change what passes.

## Plan

- **The microphone's one owner:** `hear 1` runs only after `listen`'s `[LISTEN] RECORDING` line, and `listen` records long enough to be killed while it still does.
- **The waits** of the checks after the recording come from one constant of the suite, long enough for such a load. No check's expected text changes.
- **The recording's checks** stay as they are: `OVERFLOWS 0` included.

## Acceptance criteria

- **QEMU x86, 4 CPUs, TCG:** the `listen` suite passes 20 runs in a row beside a build loop, the machine and its load named (MC-12.1).
- **Without the load,** the suite passes as before, and takes no longer when nothing is slow.

## Related

[000-DRV-0066](../issues-done/000-DRV-0066-the-ac97-capture-ring-overflows.done), [000-APP-0114](000-APP-0114-hear-waits-for-a-whole-command.md).
