# 176-KRN-0103 — `check`, `bench` and `kbench` stop when their console is gone

**Type:** `libmind`, the test and performance tools · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; the `bench` suite passes on x86; the gate left) · **Blocked by:** — · **Main task:** [176](176-test-and-performance-utilities.md) · **Constitution:** MC-6.5

## Problem

The tools track (requests-KRN.md, 2026-10-10, from the walk through wm's menu in [000-APP-0056](../issues-done/000-APP-0056-every-entry-of-wms-menu-reacts.done)): `check`, `bench` and `kbench` run from wm's menu in a `console` window. Closing the window ends `console`, but not the program it ran:

- `mind::output::send` stops sending when the launcher's endpoint answers `Peer`, and the program goes on.
- A full `kbench` or `bench` then measures for minutes with nobody watching it, and takes processor time from whatever the user does next.
- `console` cannot end its programs: ending another program is the shell's, which asks the user first.

The `wm` suite waits for the three to end before it closes their window (`RUN_TO_END`).

## Plan

The second of the request's two proposals: the tools stop themselves; libmind changes no program's behaviour.

- **`mind::output::gone()`:** the launcher lent an endpoint for the output (`SLOT_CONSOLE`) and it is gone. It is known at the first output after it went, when the send answers `Peer`.
- **The three tools** look at it after each line they show (`bench::out::Log::both`): they write their log with a line saying they stopped because their console is gone, print it to the kernel's log, and end with exit status 3.

## Acceptance criteria

- [ ] **`check`'s kernel group (`bench` suite, x86 and aarch64):** `kbench` started with an endpoint of `check`'s as its console sends four pieces of output; `check` drops the endpoint, and `kbench` ends within 30 s (`a tool's console closes`).
- [ ] **The `wm` suite** closes the three tools' windows without waiting, with `RUN_TO_END` empty: the tools track's part ([requests-APP.md](requests-APP.md) gets a note).

## Related

[requests-APP.md](requests-APP.md) (the note), `libmind/src/output.rs` (issue 162), `bench/src/out.rs`.
