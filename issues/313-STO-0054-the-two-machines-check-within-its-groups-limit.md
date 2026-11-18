# 313-STO-0054 — The two machines' check within its gate group's limit: one CPU each, and its time logged

**Type:** test (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open (2026-11-18) · **Blocked by:** — · **Main task:** [313](313-persistent-actors.md) · **Roadmap:** track B · **Constitution:** MC-12.1

## Problem

Round 148 ran "aarch64: files and the block store" on acc-2, an 8-core machine, beside another lane. It ended at the operation's limit, 3500 s, with no result line (KRN's remark, `STO/2026-11-18T0941Z-KRN-…` on `internal-dev-messages`). The same group then passed alone on x32 in 1019 s; in round 146, before the check, it took 1002 s.

The group's logs on acc-2 were not read. KRN's guess is that `tally_two_machines_check`, with two guests of 2 CPUs each under TCG beside the other lane, pushes the group past the limit there. The check gives no time of its own, so no log says what it costs.

## Plan

- **The two machines get one CPU each,** half the load the check adds beside another lane.
- **The check logs its time** (`TIME: the two machines' check took … s`), so that the next gate's logs say what it costs on each machine.
- **If a gate on acc-2 still ends at the limit,** its logs are read before anything else changes.

## Acceptance criteria

- The `store` suite passes in this container with the check on one CPU each, its time logged.
- The next round's gate passes the store groups, and their logs show the check's time.

## Related

[313](313-persistent-actors.md), 313's move between two machines (2026-11-18).
