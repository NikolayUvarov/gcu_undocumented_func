# 045 — `sysmon` service and the monitors `top`, `memmap`, `load`, `hw` (tools F9, T1)

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Roadmap:** track G, T1 · **Blocked by:** 042, 044 (035 done)

## Problem

There is no way to watch the system beyond `ps` and `cpus`.

## Plan

- `sysmon` holds OBSERVE (from `init`) and serves `sysinfo.wit` with snapshots and history.
- `top`, `memmap`, `load`, `hw` as TUI programs on `sysinfo.wit`.

## Acceptance criteria

- QEMU suites cross-check numbers (task count = `ps`, arena = `heap`, `busy_app` ≈ 100 % of its CPU).

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F9, §4.4–4.7.
