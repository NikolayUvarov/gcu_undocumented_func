# 800-LNK-0010 — Directories below `data/link/`: paths, MKDIR, MOVE, RMDIR and their listing

**Type:** protocol, service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-10.2, MC-11.4, MC-12.1, MC-12.4

## Problem

The maintainer asked on 2026-10-20 to place files and to move them ("place files and do commands, like move"). The request was recorded by `KRN` in `requests-LNK.md`. Two things stood in the way:
- **Flat names only.** The link served single-part names, so an agent could not lay out a tree in `data/link/`: a program with its data, or a release in `scripts/release.py`'s layout.
- **No file commands.** There was no way to make a directory, move or rename a file, or remove a directory.

## Plan

1. **Names become paths** below the slave's directory ([docs/link/protocol.md](../docs/link/protocol.md), section 5):
   - plain parts joined by `/`;
   - no empty, `.` or `..` part, no leading `/`, no `\` or `:`;
   - at most 255 bytes.

   `linkproto::app::path_name` checks them before any request reaches a store (MC-11.4).
2. **New requests on the files channel:**
   - MKDIR, which makes missing parents;
   - MOVE, of a file or a directory, replacing a file only when asked;
   - RMDIR, of an empty directory or recursively;
   - LIST_DIR, whose directories end in `/`.

   New codes: NOT_EMPTY and EXISTS. An older slave refuses the requests with UNSUPPORTED (MC-12.4).
3. **`link`:**
   - It works through its client confined to `data/link/`, so `vfs_server` keeps every path inside whatever `link` checks (MC-2.3).
   - A put into a missing directory is refused at its start.
   - A recursive remove needs no heap: it walks one entry at a time.
   - Each change is logged (MC-10.2).
4. **`mindlink`:**
   - `ls [<dir>]`, `mkdir`, `mv [--force]` and `rmdir [-r]`;
   - paths in `put`, `get`, `stat` and `rm`;
   - `serve` does the same over a directory on Linux or Windows.

## Acceptance criteria

- [x] **`linkproto` over a lossy line** (`directories_over_a_lossy_line`), and **`mindlink` against its `serve`** (`directories_and_programs`). Both:
  - make `a/b/`, put a file there, move it to `c/` and delete `a/`, with `ls` at each step;
  - have `../x`, `a/../../x` and `/x` refused.
- [x] **The `link` suite in QEMU, on MIND Core:**
  - does the same;
  - gets a file back through its new path;
  - has `rmdir` of a directory that is not empty refused;
  - has the climbing names refused by `link`. They never reach `vfs_server`, so this suite does not exercise its scope's own refusal.
- [x] **The MacBook Pro over the RS-485 cable** (2026-10-21): the same steps, below.

Evidence for the configurations named only (MC-12.1).

## Progress

- **2026-10-20, written.**
  - The host tests pass.
  - **The `link` suite passed in local QEMU runs on x86** with 4 CPUs (the image built from this branch). The gate is left.
  - **The new `mindlink.exe`** was run against the Mac's older `link` over RS-485:
    - `hello` worked;
    - `ls` fell back to the old listing;
    - `mkdir` was refused with "does not serve this", as the protocol says.

- **2026-10-21, the MacBook Pro** (`fast-test` b09b7bb), from Windows through the CH340 and RS-485 at 460 800 bit/s, with `mindlink.exe`:
  - made `a/b/`, put a file into it, moved it to `c/`, and got it back with the same SHA-256;
  - had `rmdir a` refused as not empty, then removed `a` whole with `-r`;
  - `ls` showed each step;
  - had `mkdir ../x` refused.

## Related

[800-LNK-0001](../issues-done/800-LNK-0001-the-link-protocol.done), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [800-LNK-0009](800-LNK-0009-programs-started-for-the-master.md), [800-LNK-0011](800-LNK-0011-effector485.md), [docs/link/protocol.md](../docs/link/protocol.md).
