# 109-NET-0010 — A parser process per session

**Type:** network (service) · **Owner:** `NET` track · **Priority:** P3 · **Status:** in progress (made for downloads; the updater's part built as [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md), waiting for `init`'s loader client) · **Blocked by:** — (the loader's part: [109-KRN-0096](109-KRN-0096-the-loader-starts-a-helper.md)) · **Main task:** [109](../issues-done/109-session-parsers.done) · **Constitution:** MC-11.11, Appendix B.6

Split from 109, whose own criteria are met with one parser process for the system.

## Problem

B.6 gives each session its own parser. Today one `parse` process serves every program, one request at a time. It keeps nothing between requests. Still, a head crafted to take over `parse` would then be in the process that parses the next program's heads, though it would hold no authority to act with.

## Plan

- **A fresh `parse` process per session** (a download, an update check), started by the session's launcher with no client at all, not even the standard ones `loader` gives a program. That spawn is the kernel and loader change this issue waits for.
- **Ended with its session.** Nothing of one session's input reaches the next one's parser.

## Acceptance criteria

Two downloads at once have their heads parsed in two processes, each holding only its own endpoint (`stat caps`). Each process ends with its download.

## Progress

**2026-10-12, the kernel session at the maintainer's word** (the network track's task, done for it).

- **The loader** starts `parse` as a helper with nothing but the caller's endpoint ([109-KRN-0096](109-KRN-0096-the-loader-starts-a-helper.md)).
- **`parse` with `--session`** serves that endpoint alone. It ends on a message that is not a call, or after 30 s without a request. It writes nothing at all: it holds no log client, and a line in its own console made the kernel keep its output instead of the download's when it ended right after it (the kernel keeps the unread output of one ended console program), so a failed download's message was lost.
- **`mind::parse::Session`** creates the endpoint, asks the loader for the helper and ends it when dropped. Where the loader starts none, it uses the shared service.
- **`download`** parses its heads in a `Session` of its own while it holds `SLOT_PARSE`; a script without `parse` still gets "NO PARSER SERVICE". Its parser ends on every way out, a failure too.
- **Tested:** the network suite on x86 (QEMU 8.2, 4 CPUs, TCG, this session's machine) passes whole; the gate on i9 and PCU is left. `session_parsers_check`: two downloads held by the server at once, two parser processes besides the service, each with one capability, its endpoint in slot 1; once the server answers, both files arrive whole and both parsers are gone. A malformed head is refused in the download's own parser, so the shared service logs nothing of it. The shared service, killed, is restarted and serves `release` again.
- **Left:** the updater's checks still use the shared service. The updater holds no loader client and may create no endpoint; the change is the update track's ([109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md)).

**2026-10-13: a download whose parser had ended waited for ever.**
- **Seen:** on aarch64 (QEMU under TCG), the gate's 8 MiB download hung at its resume, twice: over HTTP on i9 and over HTTPS on PCU (`CONNECTION CUT AT 3145728 OF 8388608, RESUMING`, then nothing for 1200 s).
- **Why:** the first 3 MiB took longer than 30 s with no head to parse, so the session's parser ended. The resumed response's head then went to the session's endpoint, which the download had created and could still receive on. The kernel therefore saw a receiver, and the call waited for the caller itself.
- **The fix:** the session keeps a copy that calls and lends pages (`CAP_WRITE | CAP_GRANT`) and drops the one that could receive. A call to an ended parser now fails at once (`ERR_PEER`). `Session` then starts another parser and asks again, and `download` says so (`ITS PARSER HAD ENDED, ANOTHER ONE PARSES`) and ends the new one on its way out.
- **In `main`** (8ae66ed), gated on i9 (x86) and PCU (aarch64): the 8 MiB downloads over HTTP and HTTPS with a cut passed on aarch64 too.
- **Tested:** `parser_restart_check` in the network suite. A server sends the head and part of the body, the test kills the download's parser, then the server cuts. The download resumes, its new parser reads the head, and the file arrives whole.

## Related

[docs/network/airlock.md](../docs/network/airlock.md), [109-NET-0008](../issues-done/109-NET-0008-parser-service.done).
