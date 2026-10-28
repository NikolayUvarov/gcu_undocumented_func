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

## Progress (2026-10-28)

**Made on `claude/youthful-mendel-mf1soy`:**

- **The directory is `system/own/<name>/`,** not `data/own/`.
  - `system/` holds the private directories already, and every badge but the owner's finds them hidden. The user's shell and every program are among those, so no rule is needed for `data/`.
  - `own` is `vfs_server`'s fifth private directory (`zone.rs` `PRIVATE`), badge `mind::fs::BADGE_OWNERS` (8).
- **The identity** is the name the loader runs the program by:
  - only when it runs it from the booted slot or the volume's root by a bare name, or from `data/apps/` (750-KRN-0169);
  - a program run by any other path (`data/link/t/x.elf`) gets no directory;
  - two programs of one name share one.
- **`init`** gives the loader a second `vfs_server` client, badged `BADGE_OWNERS`, in its slot 7.
- **The loader, at each start:**
  - makes `system/own/<name>/` if missing;
  - asks `vfs_server` for a writable scope of it, grants it in the program's `SLOT_OWN`;
  - after the start, names the program to `vfs_server` (`vfs.wit` 2.5 `adopt`). The scope then lasts as long as the program runs, whether or not it opens it within a minute; unused scopes end in a minute as before.
  - When `vfs_server` refuses, the program starts without one and the loader logs `NO OWN DIRECTORY (why)`.
- **ABI 22:** `SLOT_OWN` 39, `SLOT_DYNAMIC` 40. `SLOT_OWN` is not in `LAUNCH_SLOTS`, so no launcher fills it. The handoff stays 21.
- **`vfs_server`:**
  - `SCOPES` 8 → 64;
  - a scope records the task that made it, and only that task may `adopt` it;
  - one already used by another task is refused.
- **libmind:** `mind::fs::own` — `given`, `read`, `write`, `list`, `remove`.
- **The tools:**
  - `check files` counts its runs in `system/own/check/runs.txt` and lists the directory (row "its own directory"), and finds `system/own` refused to its files client ("system/own/ hidden");
  - `bench` measures a 1 KiB file cycle in its own (row "own: 1 KiB file cycle") and leaves `bench.txt` there.

**The limit:** no per-program disk quota exists yet, so the boot volume's free space bounds a directory. The profile says so (MC-3.7 row).

**Left:**
- "the owner's file tools see it only as the owner asks": the shell reaches no program's directory at all yet;
- the run on both architectures in the gate.

## Related

[000-NET-0045](000-NET-0045-secrets-per-program-in-keystore.md), [750-KRN-0169](../issues-done/750-KRN-0169-the-loader-checks-an-installed-application.done), [253](253-telegram-and-whatsapp-text-client.md), [260](260-ai-agents-on-the-platform.md), [261](261-mail-client.md).
