# 176-KRN-0064 — `bench`: the components' performance

**Type:** kernel and services · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made and run in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** track A, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

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

## Progress

**2026-10-10: made, and run in QEMU x86 (`--quick`).** `bench/src/bin/bench.rs` shares `kbench`'s table (`bench/src/measure.rs`, which `kbench` now uses too).

| Group | Rows |
|---|---|
| `files` | on `ram:`, in `data/` and on `log:`: writing 2 MiB (with a flush), reading it back, and a 1 KiB file's whole life (created, written, read, removed), 10 to a sample; a volume that is missing or read-only is skipped with the reason |
| `services` | `vfs_server` (a file's attributes), `sysmon` (the memory figures, one call every 40 ms: it admits 40 a second from a client, MC-10.2), `rtc` (the time), `loader` (`inspect`), `audio_gw` (the device) |
| `crypto` | SHA-256 of 4 MiB and of 64 bytes |
| `camera` | frame intervals from the first camera at up to 640×480 for 3 s (1 s with `--quick`), with the frames the gateway skipped |

The rates go under the table: MiB/s for the files and SHA-256, small files a second, and frames a second.

- **The `bench` suite** gives the boot disk `video/synthetic`, so the gateway's test pattern stands in for a camera. It checks the rows, the rates, the camera's interval and the log.
- **First numbers (x86 TCG, not evidence of any machine):**
  - `ram:`: writes 420 KiB/s, reads 944 KiB/s, 75 small files a second;
  - round trips: `vfs_server` 733 µs, `rtc` 223 µs;
  - SHA-256 at 69 MiB/s;
  - the synthetic camera at 640×480, a frame every 30.1 ms, none skipped.
- **Worth a look.** Each 16 KiB `vfs.write` takes about 38 ms on `ram:` under TCG, far more than an IPC round trip with a lent page (0.3 ms). If the MacBook Pro shows the same proportion, that is a `vfs_server` task.

Left: the MacBook Pro's numbers (criterion 4).

## Related

[176-KRN-0062](176-KRN-0062-kbench.md), [176-KRN-0063](176-KRN-0063-check.md), `netbench`.
