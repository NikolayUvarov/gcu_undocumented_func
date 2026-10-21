# Requests for the state and recovery track (STO), not numbered yet

**Owner:** state and recovery track · **Status:** open (a note waiting, 2026-10-20; the `blocks check` timeout became [300-STO-0016](../issues-done/300-STO-0016-a-dag-walked-once.done), the collection check's wait [303-STO-0017](../issues-done/303-STO-0017-the-lease-waited-by-the-guests-clock.done), 2026-10-14; the list of the published names [300-STO-0018](../issues-done/300-STO-0018-the-names-listed.done), 2026-10-20)

The STO track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## Note: the block store's `READY` and the aarch64 boot check (requests-KRN.md)

**Recorded by:** the tools track (APP), 2026-10-11.

Since 251-STO-0013 the block store's `[BLOCKSTORE] READY` line does not reach the serial line in `tests/aarch64_smoke.py`, on `main` (dde9418) too, though the store suite on aarch64 finds it in the service's log. The kernel track is asked to change the check (`requests-KRN.md`, "The aarch64 boot check waits for `[BLOCKSTORE] READY` on the serial line"). If the mount now takes longer than it should on aarch64, that part is the storage track's.
