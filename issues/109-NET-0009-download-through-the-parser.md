# 109-NET-0009 — `download` parses nothing itself

**Type:** network (tool) · **Owner:** `NET` track · **Priority:** P2 · **Status:** open · **Blocked by:** [109-NET-0008](109-NET-0008-parser-service.md), 109-KRN-0042 (`REQUEST_PARSE`), 109-APP-0016 (the shell lends the client) · **Main task:** [109](109-session-parsers.md) · **Constitution:** MC-11.5, MC-11.11

## Problem

`download` holds a flow grant and a writable file client and parses the server's response head itself (`mind::http`).

## Plan

- `download` asks for `REQUEST_PARSE` and hands every response head to `parse`. Without a client of it, it refuses to download (`NO PARSER SERVICE`) rather than parse in its own process.
- It still frames the head itself (finds the blank line) and still checks the typed head against what it asked for.
- **Tests (the `net` suite):**
  - downloads pass as before;
  - a malformed head from the server is refused by `parse` and logged there;
  - `parse` holds no file, network, spawn or device capability.

## Acceptance criteria

The download suite passes with every head parsed in `parse`; a malformed head is refused by `parse`; `download` without the parser client refuses.

## Related

[109](109-session-parsers.md), [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done).
