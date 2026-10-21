# 351-LNK-0013 — Staging a release through the link: `mindlink put -r` and `effector485`'s `stage`

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-10-21) · **Blocked by:** — · **Main task:** [351](351-self-update.md), through [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md) · **Constitution:** MC-9.2–9.4, MC-12.1, MC-12.3

## Problem

The kernel track asked on 2026-10-21 (`requests-LNK.md`, for 351-KRN-0141, at the maintainer's request) for an update of MIND Core through the link:
- the agent puts a signed release into `data/link/rel/`;
- the updater takes it from there (`update.txt`: `source data/link/rel`), boots it on trial and confirms it.

**What it takes today:**
- **The files.** A release in `scripts/release.py`'s layout (`blobs/`, `releases/<version>/<arch>/`, `channels/`) is about 40 files and a few MiB. With `mindlink` that is one `mkdir` per directory and one `put` per file.
- **The outcome.** No single operation follows the update to its end. The machine restarts on trial, the cable's other end goes away, and the updater's verdict is in the next boot's log.

## Plan

1. **`mindlink put -r DIR [NAME]`:**
   - puts a local tree under `NAME` (by default the directory's own name);
   - makes each directory first (MKDIR; one already there is fine), then puts each file, each checked by SHA-256 as `put` checks one;
   - says how many directories, files and bytes it moved.
2. **`effector485`'s operation `stage`** (`dir`, an absolute local path; `name`, by default `rel`):
   - it notes where the log ends, then puts the tree as `put -r` does;
   - it reads the log from there until the updater gives its verdict:
     - **completed:** `[UPDATER] SLOT … CONFIRMED ON THE DISK`, after the restart;
     - **failed:** a step `REFUSED`, `INIT REFUSED THE TRIAL BOOT`, or a `ROLLBACK`, with that line as the error;
   - **across the restart** it waits for the other end to answer again, then reads the new boot's log from its start;
   - the operation's limit (`timeout_seconds`) covers the whole of it, and a cancel ends the wait.
3. **The kernel track's `linkupdate` suite** puts the tree file by file until (1) exists, and uses `put -r` after.

## Acceptance criteria

- [ ] **Host tests:**
  - `mindlink put -r` against `mindlink serve`: a tree with nested directories arrives whole, a second run over it too;
  - `effector485 stage` against `serve`: the verdict read from a log, and a restart of `serve` in the middle waited through.
- [ ] **The QEMU `link` suite** puts a tree with `put -r`.
- [ ] **The kernel track's `linkupdate` suite** uses `put -r` (351-KRN-0141).

Evidence for the configurations named only (MC-12.1). `stage` reports what the updater logs. The release's checks are the updater's (MC-12.3).

## Related

[351-KRN-0141](351-KRN-0141-an-update-through-the-link.md), [800-LNK-0010](800-LNK-0010-directories-in-data-link.md), [800-LNK-0011](800-LNK-0011-effector485.md), [docs/update/updater.md](../docs/update/updater.md), [docs/link/mindlink.md](../docs/link/mindlink.md), [docs/link/effector485.md](../docs/link/effector485.md).
