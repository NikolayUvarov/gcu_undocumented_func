# 037 — Raise the task limit for new services

**Type:** kernel · **Owner:** kernel track (coordinates `kernel/src/scheduler.rs`, `common/abi.rs`) · **Priority:** P2 · **Status:** open · **Roadmap:** track G · **Constitution:** MC-1.7

## Problem

The kernel has 20 task slots: 12 services and 8 applications. The tools track adds `sysmon`, `logd` and `ramdisk`, which would take application slots.

## Plan

- `MAX_TASKS` 20 → 32 (task tables, scheduler arrays, quota of `init`); measure the kernel arena per task and record it in the profile.
- `init` keeps the application limit (`MAX_APPS`) as policy.

## Acceptance criteria

- All suites pass; `quotas` shows the new root quota; the profile states the cost per task.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) §6.
