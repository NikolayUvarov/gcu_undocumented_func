# 040 — `sysmon` service

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 038, 039 · **Roadmap:** track G, tools plan F9 · **Constitution:** MC-5.5, MC-10.2

## Problem

Tools should not hold the OBSERVE privilege themselves, and graphs need history from before the tool was opened.

## Plan

- Boot service `sysmon` with OBSERVE from `init`, serving `idl/sysinfo.wit`: snapshots of every `STAT` class into the client's buffer; history of 300 samples every 100 ms and 600 every second (per-CPU busy, interrupts, syscalls, IPC, context switches, kernel arena use, task count); load averages 1/5/15 min.
- Preallocated buffers, fixed sampling period; a client may make at most one request per 50 ms (rate limit, MC-10.2).
- The shell gets a client endpoint and passes it to tools at launch (042).

## Acceptance criteria

- `sysmon` runs in all suites; `top`/`load` read history; a flood of requests is throttled.

## Related

[docs/tools](../docs/tools/README.md) F9.
