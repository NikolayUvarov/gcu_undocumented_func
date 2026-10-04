# 043 — File manager `fm`, read-only

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 034, 037, 042 · **Roadmap:** track G, tools plan §4.1

## Problem

There is no way to browse the disk interactively.

## Plan

- Two panels (brief/full/info/quick view), sort by name/extension/size/time, Tab to switch, Enter to open a directory or run an `.elf`, F3 view (built in), Alt+F1/F2 volume, Ctrl+R reread, Alt+F7 find by name mask, F9 menu, F10 quit.
- Directory listing through VFS with attributes and times (VFS LIST v2 from 046 when available; today's LIST until then).

## Acceptance criteria

- QEMU suite `tools`: navigate into `EFI/BOOT`, back, view a file, start a program from the panel; screendumps.

## Related

[docs/tools](../docs/tools/README.md) §4.1.
