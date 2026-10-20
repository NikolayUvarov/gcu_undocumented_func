# Request: a parser session ends before its helper does (109-KRN-0133)

**From:** KRN · **To:** NET, UPD · **Written:** 2026-10-20 · **Answers:** — · **Concerns:** `libmind/src/parse.rs` (`Session`'s drop, `end_session`); `updater/src/main.rs` (`kept_version`, `parser()`) · **Answer wanted:** yes, when one of you takes it

Recorded in `issues/requests-NET.md` and `issues/requests-UPD.md` on the kernel branch (2826ff5). They reach `main` with gate g0137, now running. Either fix ends the race.

- **What changed.** 109-KRN-0133: on a disk with boot slots, `loader` now starts parser helpers from the booted slot's copy (`MIND/<slot>/parse.elf`). Before it found no `parse.elf` and every session fell back to the shared parser.
- **The race this exposed.** Gate g0134, i9, booted from slot B: `[UPDATER] PARSER: THE SHARED SERVICE (Endpoint(Other(ERR_LIMIT)))`.
  - The updater opens one session per channel it reads, one after the other, for both slots at its start.
  - A session's drop sends the helper its stop and drops the updater's capability, but does not wait for the helper to end.
  - The helper's copy keeps the endpoint alive, and `init` gives the updater one endpoint. So the second `Endpoint::create` can be refused.
- **Proposals:**
  - NET: `Session`'s drop waits, for a bounded time, until the helper has ended (`mind::process::alive(pid)`);
  - UPD: read both channels through one session.
- **Meanwhile:**
  - the fallback to the shared parser is safe and logged;
  - the `update` suite checks the own parser on slot A only. Its slot B check comes back with the fix.
