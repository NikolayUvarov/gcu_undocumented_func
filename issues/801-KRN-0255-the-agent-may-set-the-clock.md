# 801-KRN-0255 — The Effector agent gets `rtc`'s setting client, to set the clock from the pinned server's time

**Type:** grant · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-17) · **Blocked by:** — (the agent's use of it is `EFF`'s, asked in [requests-EFF.md](requests-EFF.md)) · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-7.5, MC-5.6, MC-12.3, MC-12.6

## Problem

801's S5: a clock fault may not cost the connection or control through Effector.
- **What is done.** The agent judges commands by the server's time (801-EFF-0027), and the pinned server is reached with the clock at 2000-01-01 (801-NET-0081).
- **What is not.** The device's clock stays wrong, and everything else that reads it is wrong too:
  - the updater's expiry of a channel and a timestamp (`EXPIRED AT …`), so a clock years ahead refuses every release;
  - a certificate's validity for any server but the pinned one, and `timed`'s NTS key establishment;
  - the times in the logs.
- **The maintainer's choice (2026-11-17)** between a `timed` line in the default policy and the agent setting the clock: «агент ставит часы по дате сервера», the agent sets it from the server's date.
- **Why `KRN`.** `init` lends the agent the plain `rtc` client, which reads the clock and sets nothing (`rtc/src/main.rs` refuses `set` without `mind::rtc::BADGE_SET`).

## Plan

1. **The grant.** `init`'s `effector` arm gives `rtc` with `BADGE_SET` in slot 2, as it gives `timed` and the shell. Nothing else changes: the agent still runs only in a mode, and reads the clock through the same slot.
2. **The check.** The `effector` suite's `effector_grants_check` asks slot 2 badged 1 (`BADGE_SET`), not any `rtc` client.
3. **The docs.** [bootstrap.md](../docs/profile/bootstrap.md)'s row; [clocks.md](../docs/profile/clocks.md): who may set the clock now, and what the Effector server is trusted for. The grant is no guarantee that the clock is set: that is `EFF`'s part, not done (MC-12.3).
4. **The request to `EFF`.** The agent sets the clock from the `Date` of an answer of the pinned server:
   - only when the device's clock is more than five minutes off it;
   - never before the build's commit time, never past 2099;
   - each set logged.

## Acceptance criteria

- [ ] The `effector` suite, x86 and aarch64: the agent's slot 2 is `rtc` badged `BADGE_SET`, its other grants as before.
- [ ] `docs/profile` says who may set the clock and what the server is trusted for.
- [ ] The request to `EFF` recorded.

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0027](801-EFF-0027-commands-judged-by-the-servers-time.md), [111](111-network-time.md), [312-KRN-0239](../issues-done/312-KRN-0239-a-calendar-clock-for-blockstore.done).
