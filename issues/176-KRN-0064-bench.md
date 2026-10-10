# 176-KRN-0064 — `bench`: the components' performance

**Type:** kernel and services · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track A, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

Components have no common performance measure on a real machine: the file system on each volume, the services' round trips, the camera's frame rate.

## Plan

`bench [group …] [--quick]`, a console program, in the format of `kbench`:

| Group | What it measures |
|---|---|
| `files` | on `ram:` and in `data/` (the boot disk): writing and reading 4 MiB (MiB/s); creating, writing, reading and removing small files (files/s) |
| `services` | a round trip to `vfs_server` (a file's attributes), to `sysmon` (a statistics read), to `rtc` |
| `crypto` | SHA-256 over 4 MiB (MiB/s) |
| `camera` | frames a second from the first camera, when the launcher lends one |

`netbench` keeps measuring the network against the test server.

## Acceptance criteria

1. Every group runs on QEMU x86 and aarch64, or is skipped with its reason.
2. The table and the log `log:benchNNNN.txt` have every measurement.
3. The QEMU suite checks them.
4. On the MacBook Pro the numbers are recorded with the configuration.

## Related

[176-KRN-0062](176-KRN-0062-kbench.md), [176-KRN-0063](176-KRN-0063-check.md), `netbench`.
