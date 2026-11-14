# 801-LNK-0047 — The link's files fail when its converter came long after boot

**Type:** link (`link`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-6.4, MC-12.1

## Problem

**On the Ryzen, 2026-11-14** (`fast-test` 8c067849, a disk written that day), the RS-485 converter was plugged in about 34 min after boot:
```
   0.925 vfs_server: [VFS] SCOPE 0x100 FOR :/link (WRITABLE)
   0.935 link: [LINK] WAITING FOR A SERIAL CONVERTER
2044.318 link: [LINK] SERVING data/link/ AND THE LOG AT 460800 BIT/S
3270.236 link: [LINK] REFUSED CHANNEL 2 KIND 13: CODE 3
```
- **What failed:** every file operation over the cable (`list`, `get`, `mkdir`) was refused with "the other end's storage failed". The log, keys, typed lines and `reboot` worked.
- **What the files were:** they were there. The shell had written them into `data/link/` meanwhile.
- **After a restart** with the converter in from the start, the same files were listed and read at once.

So `link`'s file client or scope went bad while it waited for a converter, and nothing opened it again. Under 801 (S6) the cable must work whenever it is plugged in.

## Plan

1. **Find what goes bad.** Reproduce in QEMU: boot with no converter, wait (or force what the long wait does: vfs scopes ending, `vfs_server` restarting, an idle timeout), plug the emulated converter in, then `list`.
2. **`link` reopens what it needs.** A storage failure on a file operation reopens its file client or scope from `init` and retries once, logging it.
3. **A check in the `link` suite:** the converter added after the boot; then `list`, `put`, `get`.

## Acceptance criteria

- **QEMU:** with the converter added after a wait, the cable's file operations work.
- **The bench:** the same, on one machine.

## Related

[801](801-the-bench-stays-manageable.md), [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md).
