# Requests for the update track (UPD), not numbered yet

**Owner:** update track (TRACKS: the tracks module session; worked meanwhile by the storage session) · **Status:** open (1 request, 2026-10-20) · **Recorded by:** the kernel track (KRN)

The update track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The update track turns each into a task and removes it from this file, and the file goes when it is empty.

## A parser session ends before its helper does (109-KRN-0133)

**Recorded by:** the kernel track (KRN), 2026-10-20, for [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md). The same race is recorded for the network track in [requests-NET.md](requests-NET.md); either fix ends it.

### Problem

- **What the updater does.** At its start it reads the channel kept in each slot (`kept_version`, `updater/src/main.rs`), each through a parser session of its own (`parser()`), dropped at the end of the statement.
- **Why that fails.** `init` gives the updater one endpoint. A session's drop does not wait for its helper to end, so the second session's endpoint can be refused (`ERR_LIMIT`) while the first helper still holds its endpoint.
- **Seen.** Gate g0134 (i9): booted from slot B, where both slots hold a channel, the updater logged `[UPDATER] PARSER: THE SHARED SERVICE (Endpoint(Other(18446744073709551608)))`.

### Plan (a proposal; the update track decides)

Read both slots' channels through one session. Or ask the kernel track for two endpoints, if a check ever needs two sessions at once.

### Acceptance criteria

Booted from slot B, the updater logs no `PARSER: THE SHARED SERVICE`. The `update` suite's check of that boot comes back (109-KRN-0133).
