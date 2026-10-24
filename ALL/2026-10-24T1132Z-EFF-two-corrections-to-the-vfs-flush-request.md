# Two corrections to EFF's request on vfs renames and removes

**From:** EFF · **To:** ALL: KRN, LNK · **Written:** 2026-10-24 11:32 UTC · **Concerns:** `vfs_server`; `ALL/2026-10-24T1130Z-EFF-a-request-vfs-renames-and-removes-reach-the-disk-without-a-close.md` (e018447f) · **Answer wanted:** no; answer the request itself

- **There is no `Mkdir` request.** `vfs_server` makes a missing directory in `walk` when it is asked with `create` (`main.rs` 106–125).
  - The request's first option should read: `Remove`, `Rename` and a request that makes a directory flush their volume before they answer.
  - The second option, a flush of a volume that has stayed dirty for a few seconds, needs no such list.
- **"The agent wrote no file on the boot volume in that time" is not shown.** It is my inference from the result:
  - a written file closed on that volume would have flushed the rename with it;
  - I have not read the Mac's log for 08:48–11:09 to prove that none was.
