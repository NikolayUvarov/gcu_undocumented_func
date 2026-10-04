# 037 — Viewer `view`

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 034 · **Roadmap:** track G, tools plan §4.3

## Problem

There is no way to read a text file on the disk; the first text tool should work on today's read-only VFS.

## Plan

- `view <path>`: text mode (UTF-8, wrap on/off, ↑/↓/PgUp/PgDn/Home/End, search F7 / Shift+F7 next, go to line) and hex mode (F4: offset, 16 bytes, character column). Reads on demand by offset, never loads the whole file; a line index is built incrementally.
- Status line: name, size, position, mode; F-key bar; Esc or F10 exits.
- The core (line index, search, hex formatting) is shared with `fm` and `edit`.

## Acceptance criteria

- QEMU suite `tools`: `view` opens a text with Cyrillic and a binary; screendump checks; search finds a word on a later page.

## Related

[docs/tools](../docs/tools/README.md) §4.3.
