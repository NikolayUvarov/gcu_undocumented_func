# 082 — `find` and `grep` as console tools

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T3

## Problem

Search by name and content exists only inside `fm` (Alt+F7). The plan lists `find` and `grep` as console tools too ([docs/tools](../docs/tools/README.md) §2.1).

## Plan

- `find [dir] [-name mask] [-type f|d] [-size +N|-N]`: walks a volume (`A:`, `ram:`) with the application's read-only file client; masks with `*` and `?` (the `fm` mask code moves into `libmind`), case-insensitive for Latin and Cyrillic.
- `grep [-i] [-n] [-r] [-l] pattern [path...]`: literal or simple regular expressions (`.`, `*`, `^`, `$`, `[...]`), UTF-8 aware, `-i` folds Latin and Cyrillic case; binary files are reported, not printed.
- Both are console programs (`REQUEST_CONSOLE`) with bounded memory (streaming reads, lines up to 4 KiB).
- Host tests for masks and matching; QEMU `disk` or `tools` suite: `find ram: -name *.txt` and `grep -rn привет ram:` on files the test writes.

## Acceptance criteria

- The tests above pass; README and `docs/tools` describe both.

## Related

[063](../issues-done/063-file-manager-read-only.done), [066](../issues-done/066-vfs-v2-fat-write.done).
