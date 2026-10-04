# 049 — `logd` and `dmesg`

**Type:** feature · **Priority:** P1 · **Status:** open · **Blocked by:** 038 · **Roadmap:** track G, tools plan F10 · **Constitution:** MC-10.2, MC-10.6

## Problem

Messages of `init` and the services sit in per-task 4 KiB queues and are lost when read or overwritten; there is no system log with time and source.

## Plan

- Boot service `logd` serving `idl/log.wit`: a 64 KiB ring of records (monotonic time, source PID and name stamped from the IPC sender, level, text up to 200 bytes); counted gaps when the ring overwrites unread records.
- `init` and the services write to it (`mind::log`); `dmesg` shows it with source/level filters and follow mode.

## Acceptance criteria

- QEMU: boot messages of `init` and `vfs_server` appear in `dmesg` with their PIDs; a forged source name in a message is not shown as the source.

## Related

[docs/tools](../docs/tools/README.md) F10.
