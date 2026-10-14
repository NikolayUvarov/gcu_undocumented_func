# 109-UPD-0017 — The updater's checks in a parser of their own

**Type:** update (service) · **Owner:** `UPD` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** in progress (built; its check waits for `init`'s loader client) · **Blocked by:** `KRN` for a loader client that only starts helpers ([requests-KRN.md](requests-KRN.md), "A loader client for the updater's parsers") · **Main task:** [109](../issues-done/109-session-parsers.done), for [109-NET-0010](109-NET-0010-a-parser-per-session.md) · **Roadmap:** track C, D · **Constitution:** MC-11.11, Appendix B.6

From the request "The updater's checks in a parser of their own" in requests-UPD.md (recorded by the kernel track, 2026-10-12).

## Problem

Each `download` parses its heads in a `parse` process of its own, which holds only that session's endpoint and ends with it (109-NET-0010). The updater's channels, manifests and response heads still go to the shared `parse` service, so one check's input reaches the process that parses the next one's.

## Plan

- **A parser per check.** `check`, `fetch` (its stage), the reading of a slot's kept channel and the verifying of the other slot each take a `mind::parse::Session` and pass it where `SLOT_PARSE` went:
  - `read_channel` and `read_manifest` get its endpoint;
  - `http::get` uses it, which starts another parser if the first ended during a long download.
- **It ends with the check**, when the session is dropped.
- **Only with a loader client in `SLOT_LOADER`.** Without one, as today, the updater uses the shared service. A capability of another kind there is not called.
- **`KRN`'s part:** `init` lends the updater a client of `loader` that may only start helpers (`loader.wit` 1.8 `helper`, 109-KRN-0096), in `SLOT_LOADER`, and an endpoint quota of 1. The updater gets no right to start programs.

## Acceptance criteria

The `update` suite's check runs its parsing in a helper that holds only its endpoint (`stat caps`), and the helper is gone when the check ends. Until `init` lends the client, the `update` suite passes as before on the shared service.

## Progress (2026-10-14)

Built for x86_64 and aarch64 (`updater/src/main.rs`, `updater/src/source.rs`). The QEMU check waits for `init`'s loader client.

## Related

[109-NET-0010](109-NET-0010-a-parser-per-session.md), [109-KRN-0096](109-KRN-0096-the-loader-starts-a-helper.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done).
