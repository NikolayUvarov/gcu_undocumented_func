# 109-KRN-0117 — A loader client that only starts helpers, for the updater's parsers

**Type:** `loader` (`loader.wit` 1.10), `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [109](../issues-done/109-session-parsers.done) · **Constitution:** MC-3.4, MC-3.7, MC-8.5

## Problem

The updater parses each check's channel, manifest and heads in a `parse` helper of the check's own, as `download` does ([109-NET-0010](109-NET-0010-a-parser-per-session.md)), when it holds a loader client in `SLOT_LOADER` ([109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md)). `init` gives it none and no endpoint quota, so it uses the shared parser. A plain loader client would also let the updater start any program, which it must not hold (MC-3.7). The update track's request of 2026-10-14 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A badge on `loader`'s clients, `BADGE_HELPERS_ONLY`** (`loader.wit` 1.10, in `mind::loader`): with it, `helper` is served as for any client ([109-KRN-0096](109-KRN-0096-the-loader-starts-a-helper.md)), and `program`, `launch` and every call that starts a program are refused with `denied`.
2. **`init` lends the updater that client** in `SLOT_LOADER`, with an endpoint quota of 1: the session's endpoint, which the helper serves.
3. **When the updater restarts,** `init` lends it a new one.

## Acceptance criteria

- [ ] The `update` suite: the updater's check runs its parsing in a helper that holds only its endpoint (`stat caps`), and the helper is gone when the check ends.
- [ ] A `program` call with the updater's loader client is refused with `denied` (`check`'s services group, a client with the badge).
- [ ] The IDL fuzzer and the generated files follow `loader.wit` 1.10.

## Related

[109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md), [109-KRN-0096](109-KRN-0096-the-loader-starts-a-helper.md), [109-NET-0010](109-NET-0010-a-parser-per-session.md), the stall of the updater's fetch after the cut (the kernel track's message to UPD of 2026-10-14 on `internal-dev-messages`).
