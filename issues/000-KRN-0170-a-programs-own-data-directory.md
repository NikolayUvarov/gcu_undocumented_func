# 000-KRN-0170 — A program's own data directory and its identity, given by the loader by the program's name

**Type:** kernel (`loader`, `init`, `vfs_server`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — (for 253, 254, 255, 260, 261) · **Constitution:** MC-3.1, MC-3.8, MC-10.3, MC-11.9, MC-12.4

Recorded by the kernel track on 2026-10-25, for the plan [docs/communications-and-agents.md](../docs/communications-and-agents.md), at the maintainer's request.

## Problem

- **The clients need a place of their own:** `mail`, `tgtxt`, `www` and `agent` keep settings, a cache, Telegram's update offset, bookmarks, a session. 253 to 261 each name "a VFS scope of its own data directory".
- **Nothing gives one today.**
  - A service gets a private directory under `system/` by its badge (`vfs_server`'s `PRIVATE` table).
  - An application started from the shell has no directory that only it writes and that other programs cannot read.
- **A secret needs an owner.** `keystore` (000-NET-0045) can keep a token for a program only if it can tell which program asks. Today it knows services by their badges, and applications not at all.

## Plan

1. **The identity.**
   - It is the name the loader ran the program by: from the booted slot, the volume's root, or `data/apps/<name>/` (750-KRN-0169).
   - The program cannot choose it, and two programs of one name are one owner.
   - The loader passes it on as a badge, or as a name in the scope's grant (the task decides which). It lends a client so badged to each service that keeps something per program: `vfs_server` here, `keystore` in 000-NET-0045.
2. **The directory** `data/own/<name>/` (the name is this task's to settle):
   - made at the program's first start;
   - given to it as a writable VFS scope in a slot of its own, as `init` gives `effector_gw` its `data/apps/` scope;
   - no other application reads or writes it. The owner's file tools see it only as the owner asks.
   - A new slot constant is an ABI change: it waits for the next ABI version and says so there (MC-12.4).
3. **The size** is bounded by the program's disk quota where there is one; otherwise the limit is stated.
4. **Removing a program** leaves its directory until the owner removes it.

## Acceptance criteria

- **In QEMU, x86 and aarch64:**
  - a test program started by name writes and reads back in its directory;
  - another program cannot open that directory;
  - a program started again finds what it wrote;
  - two names get two directories.
- **Host tests** of the zone rule in `tests/vfs_zone_host.rs`.
- **Docs:** the profile (MC-3.1, Article 11) and docs/api name the directory and its limit.

## Related

[000-NET-0045](000-NET-0045-secrets-per-program-in-keystore.md), [750-KRN-0169](../issues-done/750-KRN-0169-the-loader-checks-an-installed-application.done), [253](253-telegram-and-whatsapp-text-client.md), [260](260-ai-agents-on-the-platform.md), [261](261-mail-client.md).
