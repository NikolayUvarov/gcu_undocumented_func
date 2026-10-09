# 109-NET-0008 — The parser service `parse`

**Type:** network (service) · **Owner:** `NET` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [109](109-session-parsers.md) · **Constitution:** MC-11.4, MC-11.5, MC-11.11, Appendix B.6

## Problem

B.6's session parser stage, "bounded bytes on input and typed messages on output; no arbitrary storage, spawn or device authorities", has no process.

## Plan

- **`parse`, a boot service** that `init` starts with its own endpoint and the system log, nothing else (109-KRN-0042).
- **`idl/parse.wit` 1.0:** `http-head` turns the head of an HTTP response (at most 8 KiB, the bytes before the blank line) into a typed record: the status, `Content-Length`, `Content-Range` and whether the body is chunked. A malformed head is refused (`malformed`), and logged.
- **The parsing code** is `mind::http::parse_head`, split from `get`, with host tests. `get` takes a `Parser`: the service, or the same code in the process for the host tests. It keeps the domain's own checks on the typed head (MC-11.5): the range starts at the offset asked for, and the length agrees with it.
- **Statelessness.** Nothing of one request is kept for the next. A crash ends one request, and `init` restarts the service.

## Acceptance criteria

`parse` holds only its endpoint and the log client (`stat caps`); a client gets typed heads from it; host tests cover the parser and `get`'s checks against a parser that lies.

## Related

[109](109-session-parsers.md), [109-NET-0009](109-NET-0009-download-through-the-parser.md), [docs/network/airlock.md](../docs/network/airlock.md).
