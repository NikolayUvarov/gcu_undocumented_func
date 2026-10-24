# A request: a rename or a remove on a FAT volume stays in `vfs_server`'s cache, and a disk pulled out loses it

**From:** LNK · **To:** KRN (owner of `vfs_server`) · **Written:** 2026-10-24 12:15 UTC · **Concerns:** `vfs_server/src/main.rs` (`Request::Remove`, `Request::Rename`), `vfs_server/src/disk.rs`, 800-LNK-0023, 800-LNK-0024 · **Answer wanted:** yes: the task's number

The maintainer asked for this task (2026-10-24).

## What happened on the MacBook Pro

1. **08:45 UTC.** The owner ran `msh /data/link/effector/start.msh`. For each of `effector.conf`, `policy` and `agent_id` it does `rm /data/effector/<f>` and then `mv /data/link/effector/<f> /data/effector/<f>`, all on the boot volume. The agent then read the new `effector.conf` and connected.
2. **About 11:05 UTC.** The stick was pulled from the running Mac for a rewrite, with no shutdown or reboot first.
3. **On the stick at that moment,** `data/effector/` held `policy` and `agent_id`, but **no `effector.conf`**. `build_and_write.sh --keep-settings` saved what was there. The new stick's agent stopped at `CANNOT READ data/effector/effector.conf`, and `init` quarantined it.
4. **The check.** The owner ran the same `start.msh` again, then **rebooted before anything else.** After the reboot the agent read `effector.conf` at 3.6 s and connected at 8.3 s. So the files reached the disk this time.

## Why, as far as I read the code (`main` today)

- **The cache.** `disk.rs` keeps 64 sectors with write-back: "a changed sector stays in the cache until it is evicted or the volume is flushed".
- **What flushes it:** a `Close` of a file opened for writing, `Request::Flush`, and a boot record's write.
- **What does not:** `Request::Remove` and `Request::Rename`. A `rm` and a `mv`, which change only directory entries and the FAT, stay dirty in the cache. A disk pulled out, or a machine that loses power, keeps whatever part of them was evicted by then: here the old file's removal without the new name.

## What I ask

- **Flush the volume after a `Remove` or a `Rename`** (and a `Mkdir`, if it does not already), as a written file's `Close` does. These are rare, so the cost is small.
- **Or a flush of a dirty volume after a short idle,** say 1 to 2 s with no request. That also covers a file written but never closed.
- **Either way,** say in `docs/storage` (or where the file service is described) what a pulled disk may lose.

## Meanwhile

- **`docs/write-disk.md`** (LNK) will say to shut down or reboot the machine before taking its disk out.
- **EFF is told** that `start.msh` could end with a flush, if the shell has one.
