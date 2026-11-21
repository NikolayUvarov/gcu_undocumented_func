# 801-EFF-0040 — The agent sets the clock from the pinned server's time

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-21) · **Blocked by:** — ([801-KRN-0255](801-KRN-0255-the-agent-may-set-the-clock.md)'s grant is in `main` since round 146) · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Constitution:** MC-5.6, MC-12.1

## Problem

KRN's request of 2026-11-17, "The agent sets the clock from the pinned server's time" in `issues/requests-EFF.md`, taken here. The maintainer chose that day that the agent sets the device's clock from the server's date, rather than a `timed` line in the default policy.

- 801-EFF-0027 judges commands by the server's time, but the device's clock stays wrong. That clock is used by:
  - the updater's expiry checks;
  - other servers' certificates and NTS;
  - the logs.
- 801-KRN-0255 gives the agent `rtc` with `BADGE_SET` in its slot 2.

## Plan

1. **The rule** (`effector/src/clock.rs`, `to_set`). From the `Date` of an answer of the pinned server (the one `clock_from` reads):
   - the clock is set when the device's clock is more than five minutes off it (`OFF_S`), or cannot be read; within five minutes it is left to `timed` or the user;
   - a `Date` before the build's commit time (`MIND_SOURCE_TIME`, `timed`'s floor) or past 2099-12-31 is not used.
2. **Each set is logged once:** `EFFECTOR: THE CLOCK SET FROM THE SERVER'S TIME: <before> -> <after>`. The offset of 801-EFF-0027 then starts again from the next answer.
3. **A refusal** (a client without the setting badge, as when the agent is started by hand) is logged once and not asked again. Commands are then judged by the server's time, as before.
4. **What it trusts** is in `docs/profile/clocks.md`, "The Effector server's time". No policy line switches it off; KRN asked for none.

## Acceptance criteria

- [ ] Host test `tests/effector_clock_host.rs`: the threshold either way, the floor, 2099, an unreadable clock.
- [ ] The `effector` suite, x86 and aarch64:
  - with the guest's clock 3 h ahead and at 2000-01-01, the clock is set at the first answer, with its log line;
  - the clock is read back with `date` within 30 s of the host's.
- [ ] On the bench (the DEXP after Windows has run, its RTC in local time): the clock right after the agent's first answer (MC-12.1).

## Related

[801-EFF-0027](801-EFF-0027-commands-judged-by-the-servers-time.md), [801-KRN-0255](801-KRN-0255-the-agent-may-set-the-clock.md), [clocks.md](../docs/profile/clocks.md).
