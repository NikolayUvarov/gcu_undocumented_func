# 109-UPD-0022 — The updater reads both slots' channels through one parser session

**Type:** update (service) · **Owner:** `UPD` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** in progress (built; checks written; tested on x86 in this session's container) · **Blocked by:** — · **Main task:** [109](../issues-done/109-session-parsers.done), for [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md) · **Roadmap:** track C, D · **Constitution:** MC-9.3, MC-11.11, Appendix B.6

From the kernel track's request "A parser session ends before its helper does (109-KRN-0133)" in requests-UPD.md (2026-10-20), which this task takes from that file. The network track's half of the same request is [109-NET-0037](109-NET-0037-a-parser-session-waits-for-its-helper.md).

## Problem

- **What the updater did at its start.** It read the channel kept in each slot (`kept_version`, `updater/src/main.rs`), each through a parser session of its own, dropped at the end of the statement.
  - `verify_other` did the same: one session for the manifest's boot files, another for the slot's channel.
- **Why that failed.** `init` gives the updater one endpoint (109-KRN-0117). A session's helper can hold it for a moment after the session ends, so the next session was refused (`ERR_LIMIT`), and the updater parsed in the shared service.
- **Seen.** Gate g0134 on i9, booted from slot B: `[UPDATER] PARSER: THE SHARED SERVICE (Endpoint(Other(18446744073709551608)))`.

## Plan

- **`kept_version` takes its caller's parser.**
  - At the start, both slots' channels are read through one session.
  - `verify_other` reads the manifest's boot files and the slot's channel through one session too.
- **A check is one session.** Its channel, manifest and response heads go through it, as before. Between checks, 109-NET-0037 makes the next session wait for the last one's helper.
- **The `update` suite:**
  - booted from slot B, it wants no `PARSER: THE SHARED SERVICE` in the updater's log;
  - `updater_parser_check` comes back ([109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md)): while the server holds a check's request for the channel, exactly one helper runs beside the shared `parse`, holding only its endpoint (`stat caps`), and it is gone with the check.

## Acceptance criteria

- [ ] Booted from slot B, the updater logs no `PARSER: THE SHARED SERVICE`. The `update` suite's check of that boot comes back (x86 and aarch64).
- [ ] `updater_parser_check` passes in the `update` suite (109-UPD-0017's criterion).

## Progress (2026-10-21)

- **The `update` suite passed** in this session's container (x86, QEMU, 4 CPUs) on `f7cab643`:
  - `updater_parser_check`: while the server held a check's request, one helper ran beside the shared `parse`, holding only its endpoint, and it was gone with the check;
  - booted from slot B after the trial, the updater's log had no `PARSER: THE SHARED SERVICE`.
- **Again on `628be403`,** the branch with `main` 61e3d1d (ABI 14), in the same container: the `update` suite passed.
- **aarch64:** "aarch64: files, network and TLS", which runs the `update` suite, passed on i7-930 on `6cbcdfe0` (Effector, exit 0).
- **Left:** the landing gate.

## Related

- [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md), [109-KRN-0117](109-KRN-0117-a-loader-client-that-only-starts-helpers.md).
- [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md): the updater's checks in a parser of their own.
