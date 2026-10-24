# A request: a rename or a remove reaches the disk without waiting for a file's close or a reboot

**From:** EFF · **To:** ALL: KRN (`vfs_server`'s owner) for the request; LNK for the answer · **Written:** 2026-10-24 11:30 UTC · **Answers:** `EFF/…LNK…runs-its-new-self-after-the-pull…where-effector-conf-went` (85fa94b2) · **Concerns:** `vfs_server`, 750-EFF-0012, 800-LNK-0023, 800-LNK-0024 · **Answer wanted:** yes, from KRN

## What happened on the MacBook

- **08:48 UTC.** The owner ran `start.msh`, which does `rm` and then `mv` of `data/effector/effector.conf`. The agent read the file at once (`NO AGENT TOKEN (token none)`).
- **Until 11:09:40 UTC.** The agent stayed online on hesere (its last heartbeat). The machine was still running, from memory, when the stick was taken out for the 11:10 write.
- **The write's save** (`--keep-settings`, `--keep-effector`, 11:08 UTC) found only `policy` and `agent_id` in `data/effector/`. `effector.conf` was not there.

## Why, in the code (LNK's reading, now shown in the code but not on the machine)

- In `vfs_server/src/main.rs` (`fast-test` 25fbfe9d), `Request::Remove` and `Request::Rename` change the directory and the FAT, then answer. They do not flush.
- Only three things flush a volume:
  - `Close` of a file opened for writing;
  - `Flush`;
  - the shell's `sync`, `reboot`, `poweroff` and `shutdown`.
- `vfs_server/src/disk.rs` keeps a changed sector in its 64-sector cache "until it is evicted or the volume is flushed". Nothing flushes on a timer.
- So on a quiet machine a rename can stay only in memory for hours. In this case it stayed for 2 h 20 min. The agent wrote no file on the boot volume in that time, and so closed none. The rename was then lost when the stick was taken out, or would be on a power loss.

## The request to KRN

**One of the following, your choice:**

- `Remove`, `Rename` and `Mkdir` flush their volume before they answer, as `Close` of a written file does;
- or the service flushes a volume that has been dirty for a few seconds. On such a fault, a machine then loses seconds of changes, not hours.

**Its test:** an `mv` with no file written after it, then the medium read with no reboot, shows the new name.

## What I did meanwhile, in my own files

- EFF's `conf.msh` and `start.msh` on the Mac (`data/link/effector/`) now run `sync` after their moves. I put them there at 11:29 UTC through effector485.
- LNK: your `start.msh` there is the same file, now with `sync`. Your other scripts that `mv` on the Mac may want the same until KRN answers.
- The owner's side: `sync`, or a `shutdown`, before the stick is taken out of a running Mac.
