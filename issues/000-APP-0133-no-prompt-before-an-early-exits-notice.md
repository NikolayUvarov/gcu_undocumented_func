# 000-APP-0133 — No prompt before the exit notice of a program that ended before it was in front

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.3

**Asked by:** NET, for 000-NET-0065 (on NET's branch), in [requests-APP.md](requests-APP.md), 2026-11-10, from round 117's gate. Numbered on 2026-11-10.

## Problem

A program with a screen starts in front (`start_front`, issue 160). When it ends before the shell's `started()` calls `control::focus`, the focus fails, and the shell prints its prompt at once: the program's lines and its exit notice come under that prompt.

```
MIND> wifi scan
STARTED PID=40 NAME=wifi FOREGROUND
MIND> [PID 40] [WIFI] NETWORK "MIND-TEST" ... JOINED
[PID 40] [WIFI] SCAN: 1 NETWORK

PID=40 EXITED. SHELL RESUMED.
MIND>
```

It is rare on `main`, and frequent with 000-KRN-0231's change, which lets an application run on an idle CPU beside the shell. The tests read the output up to the prompt after the exit notice (000-NET-0065), so they do not depend on this.

## Plan

- **When the focus fails because the program has ended,** the shell takes it as in front all the same: no prompt until its exit notice, which shows its lines and `SHELL RESUMED.` as when the focus succeeds.
- **Where no notice comes** within 2 s, the shell says the program exited and gives its prompt back, so a lost notice cannot leave it without one.
- **The shell's log** says when it met the race.

## Acceptance criteria

- **QEMU, the `shell` suite:** a program with a screen that ends at once (`wifi scan` with no station), run 8 times, has no prompt between its start and its exit notice; how often it ended before its focus is counted.
- The `shell` suite passes as before.

## Related

[requests-APP.md](requests-APP.md) (NET's request); issue 160 (a program in front from its start); 000-KRN-0231 (on KRN's branch).
