# 109-NET-0010 — A parser process per session

**Type:** network (service) · **Owner:** `NET` track · **Priority:** P3 · **Status:** open · **Blocked by:** a kernel and loader change (a spawn with no standard clients; to be requested from `KRN`) · **Main task:** [109](../issues-done/109-session-parsers.done) · **Constitution:** MC-11.11, Appendix B.6

Split from 109, whose own criteria are met with one parser process for the system.

## Problem

B.6 gives each session its own parser. Today one `parse` process serves every program, one request at a time. It keeps nothing between requests. Still, a head crafted to take over `parse` would then be in the process that parses the next program's heads, though it would hold no authority to act with.

## Plan

- **A fresh `parse` process per session** (a download, an update check), started by the session's launcher with no client at all, not even the standard ones `loader` gives a program. That spawn is the kernel and loader change this issue waits for.
- **Ended with its session.** Nothing of one session's input reaches the next one's parser.

## Acceptance criteria

Two downloads at once have their heads parsed in two processes, each holding only its own endpoint (`stat caps`). Each process ends with its download.

## Related

[docs/network/airlock.md](../docs/network/airlock.md), [109-NET-0008](../issues-done/109-NET-0008-parser-service.done).
