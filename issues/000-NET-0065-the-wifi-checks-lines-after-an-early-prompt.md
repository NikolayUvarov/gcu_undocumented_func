# 000-NET-0065 — The Wi-Fi checks read `wifi`'s lines that come after an early prompt

**Type:** tests (`wifi_join_check` and the country check of 550-NET-0035, `tests/qemu_smoke.py`) · **Owner:** `NET` · **Priority:** P2 · **Status:** open (made on `claude/000-NET-0065-wifi-lines-after-an-early-prompt`, 2026-11-10; landing left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track D · **Constitution:** MC-12.2

## Problem

Round 117's gate, candidate 6f61c855 (`main` 1198b8aa with 000-KRN-0231, 000-KRN-0232 and 308-KRN-0204), "x86: RAM above 4 GiB" on acc-2. The `net` suite failed in `wifi_join_check`, at `wifi scan`:

```
AssertionError: wifi scan
STARTED PID=11 NAME=wifi FOREGROUND
MIND> dmesg -s wifi
STARTED PID=13 NAME=dmesg FOREGROUND
MIND>
```

- **The group passed when the gate ran it again alone.**
- **The same failure came in this container** on 000-KRN-0231's branch, in 2 of 4 runs of the `net` suite. That branch lets an application run on an idle CPU after a wait.
- **No gate had failed this way before.**

## What the serial line showed

A run of the `net` suite kept every VM's serial log (the suite itself keeps none of its last VM's). The Wi-Fi VM showed this:

```
MIND> wifi status
STARTED PID=38 NAME=wifi FOREGROUND
[PID 38] [WIFI] STATUS JOINED MIND-TEST 02:00:00:57:4C:A0 CHANNEL 6 11 MB/S -42 DBM

PID=38 EXITED. SHELL RESUMED.
MIND> dmesg -s wifi
STARTED PID=39 NAME=dmesg FOREGROUND
MIND> wifi scan
STARTED PID=40 NAME=wifi FOREGROUND
MIND> [PID 40] [WIFI] NETWORK "MIND-TEST" 02:00:00:57:4C:A0 CHANNEL 6 -42 DBM WPA2 REMEMBERED JOINED
[PID 40] [WIFI] SCAN: 1 NETWORK

PID=40 EXITED. SHELL RESUMED.
MIND> dmesg -s wifi
STARTED PID=41 NAME=dmesg FOREGROUND
MIND> dmesg -s wifi
STARTED PID=42 NAME=dmesg FOREGROUND
MIND>
```

- **`wifi` has a screen and starts in front** (`start_front`, issue 160). The shell shows its lines with the exit notice.
- **When it ends before the shell takes it in front,** `control::focus` fails. The shell then prints the prompt at once, and the lines and the notice come after it: `PID=40` above.
- **`dmesg -s wifi` shows none of `wifi`'s lines,** in every step: `wifi` asks for no log client (`REQUEST_WIFI` alone), so `logd` has no record of it.
- **So the check's fallback** found the scan's lines only because they were in the next `dmesg`'s text, since the last match. It kept that text out of what it returned, and the `NETWORK` line was not in `scanned`.

The comment above `run()` names this race (the gates of ASR and NET, 2026-10-20), but its model was not what the serial line shows: it took the lines to come through the system log.

## Plan

1. **A helper, `front_command`,** sends a command that starts `wifi` and reads its output up to the prompt after the shell's exit notice (`SHELL RESUMED.`). When the first prompt comes before the notice, it waits for the notice and the prompt after it.
2. **`run()` in `wifi_join_check` uses it** and checks its line there. The `dmesg -s wifi` fallback and the `dmesg` added to `scanned` go.
3. **550-NET-0035's country check** reads `wifi join` and `wifi country` the same way.
4. **The shell's early prompt** is reported to `APP`: a program with a screen that ends before the shell takes it in front leaves the prompt above its output.

## Acceptance criteria

- The `net` suite passes on x86 and aarch64.
- The `net` suite passes on 000-KRN-0231's branch with this change, where it failed 2 times in 4 without it.

## Progress

**Made (2026-11-10)** on `claude/000-NET-0065-wifi-lines-after-an-early-prompt`: steps 1 to 3, in `tests/qemu_smoke.py`.

## Related

[000-KRN-0231](000-KRN-0231-kbenchs-ten-ms-sleep-past-40-ms-on-a-loaded-host.md) (whose change made this race frequent), [000-NET-0063](../issues-done/000-NET-0063-the-wifi-checks-dmesg-waited-8-s-on-a-loaded-host.done), [000-NET-0053](../issues-done/000-NET-0053-the-wi-fi-rejoin-check-waits-for-its-lease.done), [550-APP-0033](550-APP-0033-wifi-setup-program.md).
