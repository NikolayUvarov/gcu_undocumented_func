# 000-KRN-0252 — A log that stops answering holds no service: a line waits 5 s at most for `logd`

**Type:** `libmind` · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** — (for 801 S5) · **Roadmap:** stage V · **Constitution:** MC-6.3, MC-6.8

## Problem

Found while reading the Ryzen's `wlan` silence (801-NET-0083, DRV's report). Every boot service's `println!` also sends the line to `logd`: `libmind` `log::capture` → `write` → `log::write`.

That call has the process's IPC timeout, which is 0 outside `wire::with_timeout`, and 0 means no limit. So a `logd` that stops answering stops each service at its next line, with nothing in the log to say so:
- the station;
- the network stack;
- the Effector agent.

Under 801's S5, the machine would fall out of reach because of one service, which is exactly what the maintainer said must never happen.

## Plan

- **A line waits `WRITE_MS` (5 s) at most** for `logd` (`log::send`). It still reaches the console, as every line does.
- **A line `logd` did not take in time** sends the process's lines to the console alone for `QUIET_MS` (30 s). Then `logd` is tried again, so a hung `logd` costs a service 5 s once every 30 s, not at every line.
- **The bounds are generous** against a busy `logd`: a write is answered in milliseconds, and the gate's slowest machines wait less than a second for one.

## Acceptance criteria

1. Every crate builds on x86 and aarch64, and the gate's groups pass. They read the log throughout, so a line lost to the bound would fail them.
2. Not tested: a `logd` that stops answering. That needs a test-only `logd` that receives and never replies, and is left for later.

## Related

801-NET-0083, 000-KRN-0075 (init's calls bounded), [801](801-the-bench-stays-manageable.md).
