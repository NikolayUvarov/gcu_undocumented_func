# 109-NET-0051 — The tests' service table never takes an exited row for a live service

**Type:** tests (`tests/qemu_smoke.py`) · **Owner:** `NET` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [109](../issues-done/109-session-parsers.done) · **Constitution:** MC-12.2

**Asked by:** APP, `issues/requests-NET.md` (`main` 26f14525) and `KRN/2026-10-29T1404Z-APP-the-net-suites-parser-check-reads-a-downloads-exited-parser.md` on `internal-dev-messages`.

## Problem

- **The failure.** On the tools track's runs of the x86 `net` suite, `parser_check` failed once: `stat caps 54` answered `ERROR: NO SUCH PID`.
- **The cause.** Right after `download data/bad.bin`, `ps` listed both `23 parse IPC_WAIT`, the shared service, and `54 parse EXITED`, the download's own parser (109-NET-0010) not yet reaped. `vm.services()` maps each name to its last row, so it took 54.
- **The same race elsewhere.** Any check that reads `vm.services()` while a program of a service's name has exited and is not yet reaped can take the wrong PID.

## Plan

- `vm.services()` lets a later `EXITED` row never replace a live row of the same name.
- A name whose only rows are `EXITED` is still given, so the checks that wait for a killed service's new PID read the same as before.

## Acceptance criteria

- `parser_check` reads the shared service's capabilities whether or not a download's parser is still listed.
- The x86 `net` suite passes.

## Related

- [109-NET-0010](../issues-done/109-NET-0010-a-parser-per-session.done): the download's own parser.
