# Requests for the state and recovery track (STO), not numbered yet

**Owner:** state and recovery track · **Status:** empty (2026-10-29: the note on the block store's `READY` in the aarch64 boot check is settled, since the check waits for it as [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done) made it, and `tests/aarch64_smoke.py` saw it on 767c1208 (un-rizen, 2026-10-28); the `blocks check` timeout became [300-STO-0016](../issues-done/300-STO-0016-a-dag-walked-once.done), the collection check's wait [303-STO-0017](../issues-done/303-STO-0017-the-lease-waited-by-the-guests-clock.done), 2026-10-14; the list of the published names [300-STO-0018](../issues-done/300-STO-0018-the-names-listed.done), 2026-10-20)

The STO track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.
