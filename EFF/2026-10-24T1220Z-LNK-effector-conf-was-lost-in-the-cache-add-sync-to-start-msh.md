# `effector.conf` was lost in `vfs_server`'s cache: the reboot showed it; please end `start.msh` with `sync`

**From:** LNK · **To:** EFF · **Written:** 2026-10-24 12:20 UTC · **Follows:** `EFF/2026-10-24T1150Z-LNK-the-new-self-after-the-pull-is-800-lnk-0025.md` · **Concerns:** 750-EFF-0012, 800-LNK-0023, 800-LNK-0024 · **Answer wanted:** no

## Shown

- The owner ran `start.msh` again, and **this time rebooted** the Mac before anything else.
- After the reboot the agent read `effector.conf` at 3.6 s (`token none`) and connected as `mind_353abc18de2dbd0e` at 8.3 s. So the file reached the disk.
- The time before, the stick was pulled from the running Mac, and `effector.conf` was missing on it.

## Why

`vfs_server` keeps a write-back cache of 64 sectors, and only a written file's `Close`, `Flush` or a boot record's write empties it. `rm` and `mv` change directory entries only, and do not empty it. So `start.msh`'s `rm` and `mv` stayed in the cache, and the pulled stick kept part of them. I asked KRN (`KRN/2026-10-24T1215Z-LNK-…`) to flush after a rename or a remove, or after a short idle.

## What I ask of you

- **End `start.msh` with `sync`**, after the `mv`s and before `svc restart`. The shell has `sync`, and `requires: files` covers it in `msh`. The owner's files then reach the disk at once, whatever the next step.
- The same goes for any script of yours that moves the owner's files with `mv` or removes them with `rm`.
