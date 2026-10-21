# 109-NET-0037 — A parser session waits for its helper to end, so the next one finds its endpoint free

**Type:** network (library) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** in progress (built; checks written; tested on x86 in this session's container) · **Blocked by:** — · **Main task:** [109](../issues-done/109-session-parsers.done), for [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md) · **Roadmap:** track D · **Constitution:** MC-6.6, MC-11.11, Appendix B.6

From the kernel track's request "A parser session ends before its helper does (109-KRN-0133)" in requests-NET.md (2026-10-20), which this task takes from that file. The update track's half of the same request is [109-UPD-0022](109-UPD-0022-the-slots-channels-through-one-parser.md).

## Problem

- **What `Session`'s drop did.** `libmind/src/parse.rs`: it sent the helper its stop message and dropped this program's capability. It did not wait for the helper to end.
- **Why that refuses the next session.** The helper's receiving copy keeps the endpoint alive, and the endpoint counts against its creator's quota.
  - The kernel counts it until it reaps the ended helper (`used_endpoints`, `reap`), so not even when `mind::process::alive` already says false.
  - A program with one endpoint free, as the updater has (109-KRN-0117), gets `ERR_LIMIT` for a session opened right after another. It falls back to the shared parser.
- **Seen.** Gate g0134 on i9, the updater booted from slot B: `[UPDATER] PARSER: THE SHARED SERVICE (Endpoint(Other(18446744073709551608)))`. The tools track saw it on slot A too (its message of 2026-10-21 10:00 UTC).

## Plan

- **The drop waits.** `Session`'s drop, and the restart of a helper that ended idle, end the helper and then wait, at most 1 s, until its task has ended (`mind::process::alive`). The time it ended is kept.
- **The next session waits for the reap.** Within 1 s of that end, `Session::own` creates its endpoint again when the kernel answers `ERR_LIMIT`, every 2 ms.
  - A program that has no endpoint free and ended no helper still falls back at once.
- **A check, `netcheck sessions`.** It holds every endpoint of its quota but one, as the updater has. Then it opens ten parser sessions in a row, each with a helper of its own, parses a head in each, and drops it. The `net` suite runs it.

## Acceptance criteria

- [ ] A program with one endpoint free opens and drops ten parser sessions in a row, each with a helper of its own (`netcheck sessions` in the `net` suite, x86 and aarch64).
- [ ] The same check fails without the change (shown once, then not kept as a check).
- [ ] The `net` suite's other parser checks pass as before: `session_parsers_check`, `parser_restart_check`.

## Related

- [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md): the loader starts helpers from the booted slot's copy, which made the race possible.
- [109-NET-0010](109-NET-0010-a-parser-per-session.md): a parser process per session.
- [109-UPD-0022](109-UPD-0022-the-slots-channels-through-one-parser.md): the updater's half.
