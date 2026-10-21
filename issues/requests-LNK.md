# Requests for the link track (LNK), not numbered yet

**Owner:** link track (the assessing session, `claude/LNK-link`, TRACKS 1.8) · **Status:** open (1 request, 2026-10-21) · **Recorded by:** the kernel track (KRN)

The link track numbers its own tasks (`NNN-LNK-MMMM`), so requests from other tracks wait here. The link track turns each into a task and removes it from this file, and the file goes when it is empty.

## An update through the link: staging a release (351-KRN-0141)

**Recorded by:** the kernel track (KRN), 2026-10-21, at the maintainer's request (an update of the kernel over the link), for [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md).

### Problem

- **The plan.** A release is put into `data/link/rel/` and the updater takes it from there (`update.txt`: `source data/link/rel`, `automatic apply`). It boots the new slot on trial and confirms it.
- **What it takes today.** A release in `scripts/release.py`'s layout (`blobs/`, `releases/<version>/<arch>/`, `channels/`) is about 40 files and a few MiB. With `mindlink` that is one `mkdir` per directory and one `put` per file.
- **What the agent cannot do.** It cannot follow the update as one operation.

### Plan (a proposal; the link track decides)

1. **`mindlink put -r DIR [NAME]`:** a directory tree in one command, its directories made as needed.
2. **In `effector485`, an operation that stages a release:**
   - the tree into `data/link/rel/`;
   - then it reads the system log until the updater says it confirmed the new slot, or refused it, with the reason.
   - The cable drops while the machine restarts on trial, and the operation waits for it to come back.
3. **The kernel track's `linkupdate` suite** (351-KRN-0141) puts the tree file by file until (1) exists, and uses it after.

### Acceptance criteria

`mindlink put -r` and the operation of (2) are in the link track's suites. The kernel track's `linkupdate` suite uses `put -r`.
