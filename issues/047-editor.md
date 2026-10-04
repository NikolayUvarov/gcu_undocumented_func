# 047 — Editor `edit`

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 034, 042, 046 · **Roadmap:** track G, tools plan §4.2

## Problem

No way to create or change a text file inside the system.

## Plan

- Piece table with a line index (first limit 8 MiB), UTF-8, line endings preserved, invalid bytes kept.
- Screen: menu bar (F9), text, status line, F-key bar. Keys: arrows, Home/End, PgUp/PgDn, Ctrl+Home/End, Ctrl+←/→, Shift selects, Ctrl+C/X/V, Del/Backspace, Tab, F2 save, Shift+F2 save as, F7 search, Ctrl+F7 replace, Alt+F8 go to line, Ctrl+U / Alt+Backspace undo, Ctrl+Y redo, F10 quit with unsaved-changes dialog.
- Save: write `name.tmp`, flush, rename over the original (best effort on FAT).
- Read-only mode when started with a read-only handle; editor core tested on the host.

## Acceptance criteria

- Host tests of the buffer, undo, search/replace.
- QEMU suite `edit`: open, type Cyrillic and Latin text, save on `ram:` and on a FAT disk, reread, `fsck.fat -n` clean.

## Related

[docs/tools](../docs/tools/README.md) §4.2.
