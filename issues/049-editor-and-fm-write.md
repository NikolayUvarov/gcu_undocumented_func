# 049 — Editor `edit`, `fm` write operations, `df`, `fsck` (tools T2)

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Roadmap:** track G, T2 · **Blocked by:** 046, 048

## Problem

No way to edit text or manage files.

## Plan

- `edit`: full-screen panel editor (undo on Ctrl+U/Alt+Backspace; Ctrl+Z is the attention key), save via `name.tmp` + rename. `fm`: copy, move, delete, mkdir. `df`, `fsck`.

## Acceptance criteria

- QEMU suite `edit`: edit, save, reread after reboot, `fsck.fat -n` clean.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) §4.1, §4.2.
