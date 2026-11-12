# 000-KRN-0238 — Host tests found by name, so a track that adds one changes no shared line

**Type:** build and test (`scripts/host_tests.sh`) · **Owner:** `KRN` (the integrator) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** — · **Roadmap:** — (the landing queue, AGENTS.md section 4) · **Constitution:** MC-12.2

## Problem

- **One line names 157 host tests** in `scripts/host_tests.sh`'s loop. Every track that adds a test appends its name to that line.
- **So two such branches always conflict there.** Round 123's candidate (2026-11-12) returned three of five requests (DRV, APP, NET) for that line alone, against PRT's `sunxi_watchdog`. The file is code for the landing rule, so each returned track merges `main` and asks again: a round each.

## Plan

- **The loop takes every `tests/*_host.rs`** but those built on a line of their own, which `OWN_LINES` names: other flags (`blockstore`, `journal`, `index`, `voice`, `hostname`, `rtw89_service`), a crate (`listen`, `network_budget`), or a place of their own in the order (`tts`, `regdb`, `rtw89_station_filter`).
- **A new test then needs no line** in the file: it runs once its file is in `tests/`.
- **Nothing else changes:** the same 157 tests, built and run the same way, in the order of their names.

## Acceptance criteria

- The loop finds the same 157 tests the line named (checked by listing both).
- `scripts/host_tests.sh` passes in CI and in the gate.
- AGENTS.md or CONTRIBUTING.md need no change: neither names the line.

## Progress (2026-11-12)

**Made** on `claude/000-KRN-0238-host-tests-by-name`: `OWN_LINES` and the loop over `tests/*_host.rs`. The two lists compared in this container: the same 157 tests. **Left:** the gate.

## Related

AGENTS.md section 4 (the landing queue; a conflict in code returns a request).
