# 081 — `caps`: capabilities of a task and the derivation tree

**Type:** tool · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Blocked by:** [151](151-shell-grant-slots-13-15.md) · **Roadmap:** track G, T3 · **Constitution:** MC-3.4–3.6, MC-10.2

## Problem

The plan's `caps` tool ([docs/tools](../docs/tools/README.md) §2.2, §4.7) shows a task's capabilities, the derivation tree across tasks and what a revoke would remove. The authority graph is sensitive, so it needs a stronger right than plain observation: a program asking for system information must not see who holds what.

## Plan

- `sysmon` serves `idl/sysinfo.wit` 2.x `authority(start: u32) -> result<list<authority-entry, 128>, error>` (PID, slot, kind, rights, badge, endpoint index, node, parent; paged) only to a client whose capability carries `mind::stat::BADGE_AUTHORITY`; others get `denied`.
- `caps` in `monitor/`: a task's slots (from `caps(pid)`), the derivation tree across tasks from `authority`, and for a selected capability the subtree a revoke would remove; `mind::process::REQUEST_AUTHORITY`, which the shell answers with its authority-badged `sysmon` client (slot 13, kernel issue 151).
- Host test; QEMU `tools` suite: `caps` shows `init`'s keeper and the services' copies; a program started with only `REQUEST_SYSINFO` gets `denied` from `authority`.

## Acceptance criteria

- The tests above pass; README and `docs/tools` describe `caps` and the authority badge.

## Related

[151](151-shell-grant-slots-13-15.md), [080](../issues-done/080-ipc-tool.done).
